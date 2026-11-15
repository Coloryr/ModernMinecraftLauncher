<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue';
import { t } from './i18n';
import InspectTree from './InspectTree.vue';
import type { InspectDeclaration, InspectNode, InspectTreeNode } from './inspector';

const props = defineProps<{
  /** 把检查协议消息发给预览页的注入脚本 */
  send: (message: Record<string, unknown>) => void;
}>();
const emit = defineEmits<{ close: [] }>();

/** 当前选中的元素；还没选过时为 null */
const node = ref<InspectNode | null>(null);
/** 预览页回传的整棵元素树 */
const treeNodes = ref<InspectTreeNode[]>([]);
/** 已展开节点的 path key */
const expanded = reactive(new Set<string>());
/** 当前选中节点的 path key */
const selected = ref('');
/** 右侧面板显示哪一页 */
const tab = ref<'styles' | 'computed'>('styles');
/** 新增行内样式的草稿 */
const draftName = ref('');
const draftValue = ref('');

/** path 数组转稳定 key；根是空串 */
function keyOf(path: number[]) {
  return path.join('/');
}

function setNode(value: InspectNode) {
  node.value = value;
  selected.value = keyOf(value.path);
  // 把祖先链展开，选中的行才会出现在树里
  for (let i = 0; i < value.path.length; i += 1) expanded.add(keyOf(value.path.slice(0, i)));
}

/** 处理注入脚本回传的消息；App 原样转进来 */
function handle(data: unknown) {
  const message = data as {
    type?: string;
    path?: number[];
    nodes?: InspectTreeNode[];
    node?: InspectNode;
    inline?: InspectDeclaration[];
  } | null;
  if (!message || typeof message !== 'object') return;
  if (message.type === 'tree' && Array.isArray(message.nodes)) {
    treeNodes.value = message.nodes;
    return;
  }
  if ((message.type === 'pick' || message.type === 'node') && message.node) {
    setNode(message.node);
    return;
  }
  if (message.type === 'inline' && message.inline && node.value && keyOf(message.path ?? []) === keyOf(node.value.path)) {
    node.value = { ...node.value, inline: message.inline };
  }
}

function toggle(item: InspectTreeNode) {
  const key = keyOf(item.path);
  if (expanded.has(key)) expanded.delete(key);
  else expanded.add(key);
}

function selectNode(item: InspectTreeNode) {
  selected.value = keyOf(item.path);
  props.send({ type: 'select', path: item.path });
}

function hoverNode(item: InspectTreeNode | null) {
  props.send({ type: 'hover', path: item ? item.path : null });
}

function selectAncestor(path: number[]) {
  props.send({ type: 'select', path });
}

function inputValue(event: Event) {
  return (event.target as HTMLInputElement).value;
}

/** 行内样式跟着输入实时生效；清空值等于移除这条 */
function applyInline(declaration: InspectDeclaration, raw: string) {
  if (!node.value) return;
  const important = /!important\s*$/i.test(raw);
  const value = raw.replace(/!important\s*$/i, '').trim();
  if (!value) {
    props.send({ type: 'unstyle', path: node.value.path, name: declaration.name });
    return;
  }
  props.send({ type: 'style', path: node.value.path, name: declaration.name, value, important });
}

function removeInline(declaration: InspectDeclaration) {
  if (!node.value) return;
  props.send({ type: 'unstyle', path: node.value.path, name: declaration.name });
}

function addInline() {
  if (!node.value || !draftName.value.trim()) return;
  const raw = draftValue.value.trim();
  const important = /!important\s*$/i.test(raw);
  const value = raw.replace(/!important\s*$/i, '').trim();
  if (!value) return;
  props.send({ type: 'style', path: node.value.path, name: draftName.value.trim(), value, important });
  draftName.value = '';
  draftValue.value = '';
}

const hasRules = computed(() => (node.value?.rules ?? []).some((rule) => rule.declarations.length > 0));

onMounted(() => {
  props.send({ type: 'tree' });
});

defineExpose({ handle });
</script>

<template>
  <section class="inspector" :aria-label="t('inspector')">
    <div class="panel-head">
      <strong>{{ t('inspector') }}</strong>
      <code v-if="node" class="inspector-selector" :title="node.selector">{{ node.selector }}</code>
      <span class="grow" />
      <span v-if="node" class="inspector-size">{{ node.width }} × {{ node.height }}</span>
      <button class="ui-btn v-ghost s-sm" :aria-label="t('close')" @click="emit('close')">×</button>
    </div>

    <p v-if="node" class="inspector-path">
      <button
        v-for="(item, index) in node.ancestors"
        :key="index"
        class="crumb"
        type="button"
        @click="selectAncestor(item.path)"
      >{{ item.tag }}<span v-if="item.id" class="crumb-mark">#{{ item.id }}</span><span v-if="item.classes.length" class="crumb-mark">.{{ item.classes.join('.') }}</span></button>
    </p>

    <div class="inspector-body">
      <div class="inspector-tree">
        <InspectTree
          v-if="treeNodes.length"
          :nodes="treeNodes"
          :depth="0"
          :expanded="expanded"
          :selected="selected"
          @toggle="toggle"
          @select="selectNode"
          @hover="hoverNode"
        />
        <p v-else class="inspector-empty">{{ t('inspectorLoading') }}</p>
      </div>

      <div class="inspector-side">
        <div class="inspector-tabs">
          <button type="button" :class="{ 'is-active': tab === 'styles' }" @click="tab = 'styles'">{{ t('inspectorStyles') }}</button>
          <button type="button" :class="{ 'is-active': tab === 'computed' }" @click="tab = 'computed'">{{ t('inspectorComputed') }}</button>
        </div>

        <div class="inspector-scroll">
          <p v-if="!node" class="inspector-empty">{{ t('inspectorEmpty') }}</p>

          <template v-else-if="tab === 'styles'">
            <section class="block">
              <h4>element.style</h4>
              <div v-for="item in node.inline" :key="item.name" class="decl">
                <span class="decl-name">{{ item.name }}</span>
                <input class="decl-value" :value="item.value + (item.important ? ' !important' : '')" @change="applyInline(item, inputValue($event))" />
                <button type="button" class="decl-remove" :aria-label="t('inspectorRemove')" @click="removeInline(item)">×</button>
              </div>
              <div class="decl is-new">
                <input v-model="draftName" class="decl-name-input" :placeholder="t('inspectorProperty')" />
                <input v-model="draftValue" class="decl-value" :placeholder="t('inspectorValue')" @keydown.enter="addInline" />
                <button type="button" class="decl-add" @click="addInline">{{ t('inspectorAdd') }}</button>
              </div>
            </section>

            <section v-if="hasRules" class="block">
              <template v-for="(rule, index) in node.rules" :key="index">
                <template v-if="rule.declarations.length">
                  <h4 class="rule-selector">{{ rule.selector }}</h4>
                  <div v-for="item in rule.declarations" :key="item.name" class="decl is-rule">
                    <span class="decl-name">{{ item.name }}</span>
                    <span class="decl-value">{{ item.value }}{{ item.important ? ' !important' : '' }}</span>
                  </div>
                </template>
              </template>
            </section>
          </template>

          <template v-else>
            <section class="block">
              <h4>{{ t('inspectorComputed') }}</h4>
              <dl class="styles">
                <template v-for="item in node.styles" :key="item.name">
                  <dt>{{ item.name }}</dt>
                  <dd>{{ item.value }}</dd>
                </template>
              </dl>
            </section>
            <section v-if="node.variables.length" class="block">
              <h4>{{ t('inspectorVariables') }}</h4>
              <dl class="styles">
                <template v-for="item in node.variables" :key="item.name">
                  <dt>{{ item.name }}</dt>
                  <dd>{{ item.value }}</dd>
                </template>
              </dl>
            </section>
          </template>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
/* 固定在预览区底部：左边元素树，右边样式 / 计算值 */
.inspector {
  display: flex;
  flex: 0 0 280px;
  flex-direction: column;
  min-height: 0;
  border-top: 1px solid var(--border);
  background: var(--bg-side);
}


.inspector-selector {
  max-width: 45%;
  overflow: hidden;
  color: var(--accent);
  font-family: Consolas, "Cascadia Mono", monospace;
  font-size: 11px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.inspector-size {
  color: var(--text-dim);
  font-family: Consolas, "Cascadia Mono", monospace;
  font-size: 10.5px;
  white-space: nowrap;
}

.inspector-path {
  display: flex;
  flex: none;
  flex-wrap: nowrap;
  gap: 2px;
  margin: 0;
  padding: 4px 12px;
  overflow: hidden;
  border-bottom: 1px solid var(--border);
  white-space: nowrap;
}

.crumb {
  flex: none;
  padding: 0 2px;
  border: 0;
  background: transparent;
  color: var(--text-dim);
  font-family: Consolas, "Cascadia Mono", monospace;
  font-size: 10.5px;
  cursor: pointer;
}

.crumb:hover {
  color: var(--accent);
}

.crumb + .crumb::before {
  padding-right: 2px;
  color: var(--text-dim);
  content: "›";
}

.crumb-mark {
  color: var(--accent);
}

.inspector-body {
  display: grid;
  flex: 1;
  grid-template-columns: minmax(0, 2fr) minmax(0, 3fr);
  min-height: 0;
}

.inspector-tree {
  min-width: 0;
  padding: 6px;
  overflow: auto;
  border-right: 1px solid var(--border);
}

.inspector-side {
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
}

.inspector-tabs {
  display: flex;
  flex: none;
  gap: 4px;
  padding: 6px 10px 0;
  border-bottom: 1px solid var(--border);
}

.inspector-tabs button {
  padding: 3px 10px;
  border: 0;
  border-bottom: 2px solid transparent;
  background: transparent;
  color: var(--text-dim);
  font: inherit;
  font-size: 11.5px;
  cursor: pointer;
}

.inspector-tabs button.is-active {
  border-bottom-color: var(--accent);
  color: var(--text);
}

.inspector-scroll {
  flex: 1;
  min-height: 0;
  padding: 8px 12px 12px;
  overflow: auto;
}

.block {
  margin-bottom: 12px;
}

.block h4 {
  margin: 0 0 4px;
  color: var(--text-dim);
  font-family: Consolas, "Cascadia Mono", monospace;
  font-size: 10.5px;
  font-weight: 600;
}

.rule-selector {
  color: var(--accent-2);
}

.decl {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 22px;
  font-family: Consolas, "Cascadia Mono", monospace;
  font-size: 11px;
}

.decl-name {
  flex: none;
  color: var(--text-dim);
  white-space: nowrap;
}

.decl-value {
  flex: 1;
  min-width: 0;
  padding: 1px 4px;
  border: 1px solid transparent;
  border-radius: 4px;
  background: transparent;
  color: var(--text);
  font: inherit;
  text-overflow: ellipsis;
}

input.decl-value {
  border-color: transparent;
}

input.decl-value:hover,
input.decl-value:focus {
  border-color: var(--border);
  outline: none;
}

.decl-remove,
.decl-add {
  flex: none;
  padding: 0 5px;
  border: 0;
  border-radius: 4px;
  background: transparent;
  color: var(--text-dim);
  font: inherit;
  cursor: pointer;
}

.decl-remove:hover {
  color: var(--red);
}

.decl-add {
  color: var(--accent);
}

.decl-name-input {
  flex: none;
  width: 92px;
  padding: 1px 4px;
  border: 1px solid var(--border);
  border-radius: 4px;
  background: transparent;
  color: var(--text);
  font: inherit;
}

.decl.is-new .decl-value {
  border-color: var(--border);
}

.styles {
  display: grid;
  grid-template-columns: max-content minmax(0, 1fr);
  gap: 1px 10px;
  margin: 0;
  font-family: Consolas, "Cascadia Mono", monospace;
  font-size: 11px;
}

.styles dt {
  color: var(--text-dim);
  white-space: nowrap;
}

.styles dd {
  margin: 0;
  color: var(--text);
  overflow-wrap: anywhere;
}

.inspector-empty {
  padding: 8px;
  color: var(--text-dim);
  font-size: 11.5px;
}
</style>
