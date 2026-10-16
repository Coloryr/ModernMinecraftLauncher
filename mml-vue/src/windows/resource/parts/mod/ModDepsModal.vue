<script setup lang="ts">
// 依赖树弹窗：选一个模组，用"卡片 + 连线"把它的强制 / 推荐前置展开到叶子。
//
// 只负责画：树由 ModPane 用 useModDeps 建好传进来（数据来自模组列表的 DTO）。
// 版本范围不在这里判定 —— 范围比较在内核（依赖检测那个入口），这里只把
// "要求的范围"与"装着的版本"并排显示，缺了 / 被禁用一眼能看出来。
//
// 连线用**格子的伪元素**画，不用字符：每行前面若干 18px 的格子，纵向格子画竖线、
// 最后一格画 ├ / └（上半段竖线 + 指向右的横线），于是父子关系不靠缩进而靠线表达。
import { computed } from "vue";
import { t } from "../../../../lib/i18n";
import BaseButton from "../../../../components/ui/BaseButton.vue";
import BaseModal from "../../../../components/ui/BaseModal.vue";
import type { ModItemDto } from "../../../../lib/bindings";
import type { ModDepNode } from "../../composables/useModDeps";

const props = defineProps<{
  /** 树根对应的模组 */
  item: ModItemDto;
  /** 根自己声明的依赖树 */
  tree: ModDepNode[];
}>();

const emit = defineEmits<{
  /** 关掉弹窗 */
  (e: "close"): void;
}>();

/** 树里的一行 */
interface Row {
  /** 这一行对应的节点；根那行是自己 */
  node: ModDepNode;
  /** 是不是根（被选中的那个模组） */
  root: boolean;
  /** 每一格连线的类名：vertical / blank / elbow / elbow-last */
  cells: string[];
}

/**
 * 一层节点 → 连线格子的类名
 *
 * - `guides`: 各祖先层"后面还有兄弟"的标记，长度 = 深度
 * - `last`: 这一行是不是它那一层的最后一个兄弟
 */
function cellClasses(guides: boolean[], last: boolean): string[] {
  const cells: string[] = guides.map((continues) => (continues ? "vertical" : "blank"));
  cells.push(last ? "elbow-last" : "elbow");
  return cells;
}

/**
 * 把树拍平成行
 *
 * - `nodes`: 一层节点
 * - `guides`: 祖先层的连线标记，见 `cellClasses`
 * - `out`: 结果累加到这里
 */
function flatten(nodes: ModDepNode[], guides: boolean[], out: Row[]) {
  nodes.forEach((node, index) => {
    const last = index === nodes.length - 1;
    out.push({ node, root: false, cells: cellClasses(guides, last) });
    flatten(node.children, [...guides, !last], out);
  });
}

/** 根节点：把被选中的模组自己也画成一行 */
function rootNode(): ModDepNode {
  return {
    modId: props.item.modId || props.item.file,
    required: "",
    optional: false,
    provider: props.item,
    nested: false,
    virtual: false,
    installed: props.item.version,
    missing: false,
    children: [],
  };
}

/** 弹窗里要显示的行：根 + 展开后的前置 */
const rows = computed<Row[]>(() => {
  const out: Row[] = [{ node: rootNode(), root: true, cells: [] }];
  flatten(props.tree, [], out);
  return out;
});

/** 节点卡片的主标题：有条目就用它的名字，没有就用 modid */
function titleOf(row: Row): string {
  if (row.root) return row.node.provider?.name || props.item.file;
  const provider = row.node.provider;
  return provider ? provider.name || provider.modId || row.node.modId : row.node.modId;
}
</script>

<template>
  <BaseModal :title="t('resource.depTreeTitle', { name: item.name || item.file })" :width="560"
    :closable="false" @close="emit('close')">
    <p v-if="!tree.length" class="empty-tip">{{ t("resource.depTreeNone") }}</p>
    <div v-else class="tree-wrap">
      <div v-for="(row, i) in rows" :key="`${row.node.modId}-${i}`" class="tree-row">
        <span v-for="(cls, k) in row.cells" :key="k" class="tree-guide" :class="cls" />

        <div class="tree-node" :class="{ 'tree-root': row.root, 'tree-missing': row.node.missing }">
          <!-- 没图标就整块省掉，不留占位字形 -->
          <img v-if="row.node.provider?.icon" class="tree-icon" :src="row.node.provider.icon" alt="" />

          <div class="tree-body">
            <div class="tree-title">
              <span class="tree-name">{{ titleOf(row) }}</span>
              <span v-if="!row.root && titleOf(row) !== row.node.modId" class="tree-modid">
                {{ row.node.modId }}
              </span>
            </div>
            <div class="tree-meta">
              <span v-if="row.root" class="tree-ver">{{ row.node.installed }}</span>
              <template v-else>
                <span v-if="row.node.optional">{{ t("resource.depTreeOptional") }}</span>
                <span v-if="row.node.nested">{{ t("resource.modBuiltin") }}</span>
                <!-- 游戏 / 加载器提供的虚拟前置：没有模组条目，但不算缺失 -->
                <span v-if="row.node.virtual">{{ t("resource.depTreeVirtual") }}</span>
                <!-- 只有禁用包提供它时，依赖检测会报"前置被禁用" -->
                <span v-if="row.node.provider?.disable">{{ t("resource.depKindDisabled") }}</span>
                <span v-if="row.node.missing" class="tree-bad">{{ t("resource.depKindMissing") }}</span>
                <span v-if="row.node.installed">
                  {{ t("resource.depInstalled", { version: row.node.installed }) }}
                </span>
                <span v-if="row.node.required !== '*'">
                  {{ t("resource.depRequired", { range: row.node.required }) }}
                </span>
              </template>
            </div>
          </div>
        </div>
      </div>
    </div>

    <div class="modal-actions">
      <BaseButton variant="accent" @click="emit('close')">{{ t("actions.confirm") }}</BaseButton>
    </div>
  </BaseModal>
</template>

<style scoped>
/* 树可能又深又宽：纵向按窗口高度收，横向让它滚 */
.tree-wrap {
  display: flex;
  flex-direction: column;
  max-height: 52vh;
  overflow: auto;
}

.tree-row {
  display: flex;
  align-items: stretch;
  width: max-content;
}

/* 连线格子：竖线画在格子中间，横线从中间接到右边的卡片 */
.tree-guide {
  position: relative;
  flex: 0 0 18px;
  width: 18px;
}

.tree-guide.vertical::before {
  content: "";
  position: absolute;
  left: 8px;
  top: 0;
  bottom: 0;
  border-left: 1px solid var(--border);
}

.tree-guide.elbow::before {
  content: "";
  position: absolute;
  left: 8px;
  top: 0;
  bottom: 0;
  border-left: 1px solid var(--border);
}

/* 最后一个兄弟：竖线只画到中间，下边不再延伸 */
.tree-guide.elbow-last::before {
  content: "";
  position: absolute;
  left: 8px;
  top: 0;
  height: 50%;
  border-left: 1px solid var(--border);
}

.tree-guide.elbow::after,
.tree-guide.elbow-last::after {
  content: "";
  position: absolute;
  left: 8px;
  top: 50%;
  width: 10px;
  border-top: 1px solid var(--border);
}

/* 卡片：图标 + 名字 + 版本口径 */
.tree-node {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  margin: 3px 0;
  padding: 4px 10px 4px 6px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg-card);
}

.tree-root {
  border-color: var(--accent-border);
}

.tree-missing {
  border-style: dashed;
}

.tree-icon {
  width: 22px;
  height: 22px;
  border-radius: 5px;
  flex-shrink: 0;
  object-fit: contain;
}

.tree-body {
  min-width: 0;
}

.tree-title {
  display: flex;
  align-items: baseline;
  gap: 8px;
}

.tree-name {
  color: var(--text);
  font-size: 12.5px;
  font-weight: 600;
  word-break: break-all;
}

.tree-modid,
.tree-ver {
  color: var(--text-dim);
  font-size: 11.5px;
}

.tree-meta {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 3px 10px;
  color: var(--text-dim);
  font-size: 11.5px;
}

.tree-bad {
  padding: 0 6px;
  border: 1px solid var(--border);
  border-radius: 6px;
  white-space: nowrap;
}
</style>
