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
  <div class="dep-node box-border flex items-center gap-2 overflow-hidden rounded-card border border-border bg-card px-[9px] py-1.5" :class="{
    'dep-node-root': props.data.node.root,
    'dep-node-mandatory': props.data.node.mandatory,
    'dep-node-optional': !props.data.node.root && !props.data.node.mandatory,
    'dep-node-missing': props.data.node.missing,
    'dep-node-disabled': props.data.node.disabled,
    'dep-node-virtual': props.data.node.virtual,
  }" :style="{ width: NODE_W + 'px', height: NODE_H + 'px' }" v-tip="tip">
    <Handle type="target" :position="Position.Left" class="dep-handle h-px w-px min-h-0 min-w-0 border-0 bg-transparent opacity-0" />

    <img v-if="props.data.node.icon" class="dep-node-icon h-6 w-6 shrink-0 rounded-ctl object-contain" :src="props.data.node.icon" alt="" />
    <div class="dep-node-body flex min-w-0 flex-1 flex-col leading-[1.25]">
      <span class="dep-node-name overflow-hidden text-ellipsis whitespace-nowrap text-body font-semibold text-fg">{{ props.data.node.label }}</span>
      <span class="dep-node-sub overflow-hidden text-ellipsis whitespace-nowrap text-caption text-dim">{{ sub }}</span>
    </div>

    <!-- 装没装：装了打勾，装了但版本对不上（或只有禁用包提供）打感叹号 -->
    <span v-if="props.data.mark" class="dep-node-mark inline-flex h-[18px] w-[18px] shrink-0 items-center justify-center rounded-full" :class="props.data.mark">
      <GlyphIcon :name="props.data.mark === 'ok' ? 'check' : 'alert'" :size="11" :weight="2.8" />
    </span>

    <Handle type="source" :position="Position.Right" class="dep-handle h-px w-px min-h-0 min-w-0 border-0 bg-transparent opacity-0" />
  </div>
</template>

<style scoped>
/* 状态类留在 CSS：边框色按「可选 → 必装 → 游戏/加载器 → 根 → 缺失」的顺序叠，
   同权重时后写的赢，改成工具类就没有这个顺序了。排版在模板上。 */
.dep-node-optional {
  border-color: var(--border);
}

.dep-node-mandatory {
  border-color: var(--accent-border);
}

/* 游戏 / 加载器提供的前置：单独一色，它不是模组，装不装由运行环境决定 */
.dep-node-virtual {
  border-color: var(--yellow);
  background: var(--bg-raised);
}

/* 图的根：浅主色底纹，不占边框色 —— 边框色一档一个含义 */
.dep-node-root {
  background-color: var(--bg-raised);
  background-image: linear-gradient(var(--accent-soft), var(--accent-soft));
}

/* 缺失：红虚线框盖过上面几档 */
.dep-node-missing {
  border-color: var(--red);
  border-style: dashed;
}

.dep-node-disabled {
  opacity: 0.55;
}

/* 装没装的标记：绿勾 / 红叹号，颜色是非 token 的 color-mix */
.dep-node-mark.ok {
  background: color-mix(in srgb, var(--green) 18%, transparent);
  color: var(--green);
}

.dep-node-mark.warn {
  background: color-mix(in srgb, var(--red) 16%, transparent);
  color: var(--red);
}
</style>
