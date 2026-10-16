<script setup lang="ts">
// 依赖图里的一个节点：图标 + 名字 + 版本，右边一枚"装没装"的标记。
//
// 尺寸必须与布局常量一致（NODE_W / NODE_H）—— dagre 就是按那个尺寸算层间距与同层间距的。
// 左右两个 Handle 是 vue-flow 的连线锚点，本身不画出来（见下面的样式）。
import { computed } from "vue";
import { Handle, Position } from "@vue-flow/core";
import { t } from "../../../../lib/i18n";
import GlyphIcon from "../../../../components/ui/GlyphIcon.vue";
import { NODE_H, NODE_W, type ModDepGraphNode } from "../../composables/modDepGraph";

// vue-flow 往节点组件上塞的属性不止 data / selected，别让它们落到 DOM 上
defineOptions({ inheritAttrs: false });

const props = defineProps<{
  /** vue-flow 传下来的节点数据：图里的节点 + 标记 + 问题种类 */
  data: {
    /** 图里的节点 */
    node: ModDepGraphNode;
    /** 装没装：ok 打勾、warn 打感叹号、空串不打 */
    mark: "ok" | "warn" | "";
    /** 打感叹号的原因，没有时为空 */
    problem?: "missing" | "mismatch" | "disabled";
  };
  /** 是否被选中，由 vue-flow 给 */
  selected?: boolean;
}>();

/** 第二行的文字：装了给版本号，没装给版本要求 */
const sub = computed(() => {
  const node = props.data.node;
  if (node.version) return node.version;
  const range = node.required[0];
  return range ? t("resource.depRequired", { range }) : "";
});

/** 悬停提示：图里放不下的口径都收在这里 */
const tip = computed(() => {
  const node = props.data.node;
  const lines = [node.label];
  if (node.modId && node.modId !== node.label) lines.push(node.modId);
  if (node.version) lines.push(t("resource.depInstalled", { version: node.version }));
  for (const range of node.required) lines.push(t("resource.depRequired", { range }));
  if (node.root) {
    lines.push(t("resource.depGraphRoot"));
  } else {
    // 边框颜色说明不了"这颜色是哪一档"，提示里补一句
    lines.push(node.mandatory ? t("resource.depGraphMandatory") : t("resource.depGraphOptional"));
  }
  if (props.data.problem === "mismatch") lines.push(t("resource.depKindMismatch"));
  if (props.data.problem === "disabled" || node.disabled) lines.push(t("resource.depKindDisabled"));
  if (node.missing) lines.push(t("resource.depKindMissing"));
  if (node.virtual) lines.push(t("resource.depTreeVirtual"));
  if (node.nested) lines.push(t("resource.modBuiltin"));
  // 双击能换目标这件事图上没别的地方写，塞进提示里
  if (!node.root && node.installed && !node.virtual) lines.push(t("resource.depGraphReroot"));
  return lines.join("\n");
});
</script>

<template>
  <div class="dep-node" :class="{
    'dep-node-root': props.data.node.root,
    'dep-node-mandatory': props.data.node.mandatory,
    'dep-node-optional': !props.data.node.root && !props.data.node.mandatory,
    'dep-node-missing': props.data.node.missing,
    'dep-node-disabled': props.data.node.disabled,
    'dep-node-virtual': props.data.node.virtual,
  }" :style="{ width: NODE_W + 'px', height: NODE_H + 'px' }" v-tip.mouse="tip">
    <Handle type="target" :position="Position.Left" class="dep-handle" />

    <img v-if="props.data.node.icon" class="dep-node-icon" :src="props.data.node.icon" alt="" />
    <div class="dep-node-body">
      <span class="dep-node-name">{{ props.data.node.label }}</span>
      <span class="dep-node-sub">{{ sub }}</span>
    </div>

    <!-- 装没装：装了打勾，装了但版本对不上（或只有禁用包提供）打感叹号 -->
    <span v-if="props.data.mark" class="dep-node-mark" :class="props.data.mark">
      <GlyphIcon :name="props.data.mark === 'ok' ? 'check' : 'alert'" :size="11" :weight="2.8" />
    </span>

    <Handle type="source" :position="Position.Right" class="dep-handle" />
  </div>
</template>

<style scoped>
.dep-node {
  display: flex;
  align-items: center;
  gap: 8px;
  box-sizing: border-box;
  padding: 6px 9px;
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: 9px;
  background: var(--bg-card);
}

/* 边框按"必装 / 可选"两档配色；下面的根与缺失都要盖过它俩，
   所以顺序不能动：同权重时后写的赢。
   可选 = 中性灰（等于默认边框），必装 = 主色 —— 一眼先看见必装的那些 */
.dep-node-optional {
  border-color: var(--border);
}

.dep-node-mandatory {
  border-color: var(--accent-border);
}

/* 游戏 / 加载器提供的（minecraft、forge 这类，统称虚拟前置）：单独一色。
   它不是模组，装不装由运行环境决定，跟"必装 / 可选"是两回事 */
.dep-node-virtual {
  border-color: var(--yellow);
  background: var(--bg-raised);
}

/* 图的根，即当前打开的这个模组：用浅主色**底纹**标记，不占边框色。
   边框色一档一个含义（必装 / 可选 / 游戏加载器），选中还要用它，根再插一脚就分不清了 */
.dep-node-root {
  background: var(--accent-soft);
}

/* 缺失：红虚线框盖过上面几档 —— 它一定是必装，红色说明"还没装" */
.dep-node-missing {
  border-color: var(--red);
  border-style: dashed;
}

.dep-node-disabled {
  opacity: 0.55;
}

.dep-node-icon {
  width: 24px;
  height: 24px;
  border-radius: 6px;
  flex-shrink: 0;
  object-fit: contain;
}

.dep-node-body {
  display: flex;
  flex-direction: column;
  min-width: 0;
  flex: 1;
  line-height: 1.25;
}

.dep-node-name {
  overflow: hidden;
  color: var(--text);
  font-size: 12px;
  font-weight: 600;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.dep-node-sub {
  overflow: hidden;
  color: var(--text-dim);
  font-size: 10.5px;
  white-space: nowrap;
  text-overflow: ellipsis;
}

/* 装没装的标记：一枚小圆牌，绿勾 / 红叹号 */
.dep-node-mark {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  flex-shrink: 0;
  border-radius: 50%;
}

.dep-node-mark.ok {
  background: rgba(22, 163, 74, 0.18);
  color: var(--green);
}

.dep-node-mark.warn {
  background: rgba(220, 38, 38, 0.16);
  color: var(--red);
}

/* 连线锚点不画出来：节点四边就是锚点该在的位置，露出来反而碍眼 */
.dep-handle {
  width: 1px;
  height: 1px;
  min-width: 0;
  min-height: 0;
  border: none;
  background: transparent;
  opacity: 0;
}
</style>
