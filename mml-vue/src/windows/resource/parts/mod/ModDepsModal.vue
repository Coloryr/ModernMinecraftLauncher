<script setup lang="ts">
// 依赖图弹窗：画与交互全部交给 @vue-flow/core —— 拖节点、滚轮缩放、拖画布平移、
// 点连线都是它自带的。
//
// **关系在后端**：节点与连线由 `resource_mods_dep_tree` 算好（见 AGENTS.md §12.1），
// 这个组件打开 / 换目标时向后端要一张，交给 dagre 布局（composables/modDepGraph.ts）
// 后翻成 vue-flow 的 nodes / edges。这里只做三件事：
// - 取图、布局、翻成 vue-flow 的两张表，节点上的"装没装"标记直接读后端给的 problem；
// - 点连线时在鼠标旁说明"谁需要谁、要什么版本"；
// - 一个重置视图的按钮，拖乱了能一键回到全图可见。
import { computed, nextTick, ref, watch } from "vue";
import {
  MarkerType,
  VueFlow,
  useNodesInitialized,
  useVueFlow,
  type Edge,
  type Node,
} from "@vue-flow/core";
import { Background } from "@vue-flow/background";
import "@vue-flow/core/dist/style.css";
import "@vue-flow/core/dist/theme-default.css";
import { t, tErr } from "../../../../lib/i18n";
import BaseButton from "../../../../components/ui/BaseButton.vue";
import BaseModal from "../../../../components/ui/BaseModal.vue";
import type { ModDepEdgeDto, ModItemDto } from "../../../../lib/bindings";
import { type ModDepGraph, type ModDepGraphNode } from "../../composables/modDepGraph";
import ModDepNode from "./ModDepNode.vue";

const props = defineProps<{
  /** 图根对应的模组 */
  item: ModItemDto;
  /** 取依赖图：后端建图 + 本地布局，失败抛错 */
  loadTree: (item: ModItemDto) => Promise<ModDepGraph>;
}>();

const emit = defineEmits<{
  /** 关掉弹窗 */
  (e: "close"): void;
  /** 双击某个节点：请求换成以它为目标，载荷是它的 modid */
  (e: "target", key: string): void;
}>();

/** vue-flow 的实例：重置视图要调它的 fitView */
const { fitView } = useVueFlow();
/** 所有节点是否都量好尺寸了；换目标后要等它，见 fitAfterLayout */
const nodesReady = useNodesInitialized();

/** 当前这张图；还没取回来时是 null */
const graph = ref<ModDepGraph | null>(null);
/** 正在取图 */
const loading = ref(false);
/** 取图失败的原因，空串表示没失败 */
const error = ref("");
/** 取图代次：连续换目标时丢弃旧一发的返回 */
let seq = 0;

/** 节点标记：装了打勾；装着但版本合不上（或只有禁用包提供）打感叹号 */
function markOf(node: ModDepGraphNode): "ok" | "warn" | "" {
  if (node.root || !node.installed) return "";
  return node.problem === "mismatch" || node.problem === "disabled" ? "warn" : "ok";
}

const flowNodes = computed<Node[]>(() =>
  (graph.value?.nodes ?? []).map((node) => ({
    id: node.key,
    type: "mod",
    position: { x: node.x, y: node.y },
    data: { node, mark: markOf(node), problem: node.problem ?? undefined },
  })),
);

const flowEdges = computed<Edge[]>(() =>
  (graph.value?.edges ?? []).map((edge) => ({
    id: edge.id,
    source: edge.from,
    target: edge.to,
    // 不指定 type 就是 vue-flow 的默认边，即贝塞尔曲线；
    // smoothstep 是直角折线（带圆角），之前误用它，整张图看着像电路图
    // 加载器 → minecraft 那条走黄色（与虚拟前置的边框同色）；环上的回边走主色虚线、
    // 可选前置走点线。都写在 style 里，免得被主题默认样式盖掉
    style: edge.loader
      ? { stroke: "var(--yellow)" }
      : edge.backward
        ? { stroke: "var(--accent)", strokeDasharray: "6 4" }
        : edge.optional
          ? { strokeDasharray: "3 3" }
          : undefined,
    // 箭头颜色跟着线走：默认那枚是主题色，画在黄线上会很跳
    markerEnd: edge.loader
      ? { type: MarkerType.ArrowClosed, color: "var(--yellow)" }
      : MarkerType.ArrowClosed,
    data: { edge },
  })),
);

/** 画布外层：处理"从连线上起手平移"时要找里面的 vue-flow 视口 */
const flowEl = ref<HTMLElement | null>(null);

/**
 * 从连线上按下也能平移画布
 *
 * vue-flow 给每条线的组都挂了 `nopan`，它的 d3 平移筛选因此拒绝从连线起手。
 * 这里把这次按下按"落在画布上"再派发一次 mousedown，交给 d3 的平移接管：
 * 拖动时 d3 会把随后的 click 一并吞掉，所以轻点连线仍然会触发 edge-click 弹说明。
 *
 * - `event`: 画布内按下的 mousedown
 */
function onFlowMouseDown(event: MouseEvent) {
  if (event.button !== 0) return;
  const target = event.target as Element | null;
  if (!target?.closest(".vue-flow__edge")) return;
  const viewport = flowEl.value?.querySelector<HTMLElement>(".vue-flow__viewport");
  if (!viewport) return;
  viewport.dispatchEvent(
    new MouseEvent("mousedown", {
      bubbles: true,
      cancelable: true,
      view: window,
      clientX: event.clientX,
      clientY: event.clientY,
      button: 0,
      buttons: 1,
    }),
  );
}

/** 点了线之后浮在鼠标旁边的那句话；空串 = 不显示 */
const popupText = ref("");
/** 浮层位置，单位 px，视口坐标（浮层是 fixed 的） */
const popupPos = ref({ x: 0, y: 0 });
/** 浮层元素，摆位时要量它的宽高 */
const popupEl = ref<HTMLElement | null>(null);

/** 节点键 → 节点 */
const nodeOf = computed(() => {
  const map = new Map<string, ModDepGraphNode>();
  for (const node of graph.value?.nodes ?? []) map.set(node.key, node);
  return map;
});

/** 节点键 → 显示名 */
const labelOf = computed(() => {
  const map = new Map<string, string>();
  for (const node of graph.value?.nodes ?? []) {
    map.set(node.key, node.label || node.modId || node.key);
  }
  return map;
});

/** 线上文字里的"版本"段；不知道版本就不写这一段 */
function versionPart(node: ModDepGraphNode): string {
  return node.version ? t("resource.depGraphVersion", { version: node.version }) : "";
}

/**
 * 一条线怎么念
 *
 * 以**被依赖的那个**为主语："Fabric API 版本 0.15.0 是 Sodium 的前置"，
 * 比"谁需要谁"更贴依赖表的读法。游戏 / 加载器另说一套：它们是"工作在什么版本上"，
 * 不是"前置"。环再按一句"循环依赖"。
 *
 * - `edge`: 点中的那条线
 *
 * # 返回值
 *
 * 一句话
 */
function edgeTextOf(edge: ModDepEdgeDto): string {
  const from = labelOf.value.get(edge.from) ?? edge.from;
  const to = nodeOf.value.get(edge.to);

  const text = to?.virtual
    ? t("resource.depGraphWorksOn", {
      from,
      target: to.label || to.modId || to.key,
      version: versionPart(to),
    })
    : t(edge.optional ? "resource.depGraphIsOptionalDep" : "resource.depGraphIsDep", {
      from,
      target: labelOf.value.get(edge.to) ?? edge.to,
      version: to ? versionPart(to) : "",
    });

  return edge.backward ? text + " " + t("resource.depGraphCycle") : text;
}

/** 指针落点：鼠标直接用 client 坐标，触摸用触点 */
function pointerOf(event: MouseEvent | TouchEvent): { x: number; y: number } {
  if (event instanceof MouseEvent) return { x: event.clientX, y: event.clientY };
  const touch = event.changedTouches[0] ?? event.touches[0];
  return touch ? { x: touch.clientX, y: touch.clientY } : { x: 0, y: 0 };
}

/** 收起浮层 */
function hidePopup() {
  popupText.value = "";
}

/**
 * 把说明浮在鼠标旁边
 *
 * 先按落点右下摆，量到实际宽高后再往回收 —— 与 lib/tip.ts 同一套口径：
 * 右缘放不下就翻到左边，底边放不下就翻到上边。
 *
 * - `event`: 触发这次显示的指针事件
 * - `text`: 要显示的那句话
 */
async function showPopup(event: MouseEvent | TouchEvent, text: string) {
  const { x, y } = pointerOf(event);
  popupText.value = text;
  popupPos.value = { x: x + 14, y: y + 18 };

  await nextTick();
  const el = popupEl.value;
  if (!el) return;
  let left = x + 14;
  let top = y + 18;
  if (left + el.offsetWidth > window.innerWidth - 8) left = Math.max(8, x - el.offsetWidth - 14);
  if (top + el.offsetHeight > window.innerHeight - 8) top = Math.max(8, y - el.offsetHeight - 18);
  popupPos.value = { x: left, y: top };
}

/** 向后端取图并布局；连续换目标时只有最后一发的返回算数 */
async function load(item: ModItemDto) {
  const mine = ++seq;
  loading.value = true;
  error.value = "";
  try {
    const tree = await props.loadTree(item);
    if (mine !== seq) return;
    graph.value = tree;
  } catch (e) {
    if (mine !== seq) return;
    graph.value = null;
    error.value = tErr(e);
  } finally {
    if (mine === seq) loading.value = false;
  }
}

/** 双击节点：换成以它为目标。虚拟前置与缺失的前置没有条目，不给换 */
function onNodeDoubleClick(payload: { node: { id: string } }) {
  const node = nodeOf.value.get(payload.node.id);
  if (!node || node.root || node.virtual || !node.installed) return;
  hidePopup();
  emit("target", node.key);
}

/** 点连线：在鼠标旁边说清这条依赖 */
function onEdgeClick(payload: { edge: Edge; event: MouseEvent | TouchEvent }) {
  const edge = graph.value?.edges.find((each) => each.id === payload.edge.id);
  if (!edge) return;
  void showPopup(payload.event, edgeTextOf(edge));
}

/**
 * 等节点量好尺寸再摆视图
 *
 * 刚加进来的节点还没有 bounds（量尺寸是 ResizeObserver 的异步回调），这时候 fit
 * 会按 0 尺寸算，视图是歪的 —— 换目标之后新节点一批批进来，必须等。
 * 尺寸已经在的（如"重置视图"）第一轮就过，最多等 30 帧，免得量不出来时卡住。
 */
async function fitAfterLayout() {
  for (let frame = 0; frame < 30; frame += 1) {
    await nextTick();
    if (nodesReady.value) break;
    await new Promise((resolve) => requestAnimationFrame(resolve));
  }
  void fitView({ padding: 0.16 });
}

/** 拖乱了 / 缩放过了：回到全图可见 */
function resetView() {
  hidePopup();
  void fitAfterLayout();
}

// 打开或换目标：向后端要图
watch(
  () => props.item,
  (item) => {
    hidePopup();
    void load(item);
  },
  { immediate: true },
);

// 换目标后图整张换掉，但画布的缩放 / 平移还留着上一张的值：
// 按新图重摆一次，否则新节点可能整片落在视野外
watch(graph, () => {
  hidePopup();
  void fitAfterLayout();
});
</script>

<template>
  <!-- 没有前置时不锁高度：锁了会是一个空荡荡的大面板 -->
  <BaseModal :title="t('resource.depTreeTitle', { name: props.item.name || props.item.file })" :width="960"
    :fixed-height="graph?.edges.length ? '76vh' : undefined" :closable="false" @close="emit('close')">
    <div v-if="loading" class="empty-tip">{{ t("resource.loading") }}</div>

    <div v-else-if="error" class="empty-tip">
      <p>{{ error }}</p>
      <BaseButton size="sm" @click="load(props.item)">{{ t("modpack.retry") }}</BaseButton>
    </div>

    <p v-else-if="!graph?.edges.length" class="empty-tip">{{ t("resource.depTreeNone") }}</p>

    <div v-else ref="flowEl" class="dep-flow" @mousedown.capture="onFlowMouseDown">
      <!-- 拖节点、滚轮缩放、拖画布平移全是 vue-flow 的默认行为；
           连线本身带 nopan，从线上起手的情况由 onFlowMouseDown 补一次 -->
      <VueFlow :nodes="flowNodes" :edges="flowEdges" :min-zoom="0.2" :max-zoom="2.5" :nodes-connectable="false"
        :fit-view-on-init="true" @edge-click="onEdgeClick" @node-click="hidePopup"
        @node-double-click="onNodeDoubleClick" @node-drag-start="hidePopup" @pane-click="hidePopup">
        <Background :gap="18" :size="1.1" :pattern-color="'var(--border)'" />
        <template #node-mod="nodeProps">
          <ModDepNode :data="nodeProps.data" :selected="nodeProps.selected" />
        </template>
      </VueFlow>
    </div>

    <!-- 点了线之后浮在鼠标旁边的那句话：fixed 定位，量过宽高再摆（见 showPopup） -->
    <p v-if="popupText" ref="popupEl" class="dep-edge-popup"
      :style="{ left: popupPos.x + 'px', top: popupPos.y + 'px' }">{{
        popupText }}</p>

    <div class="modal-actions">
      <BaseButton @click="resetView">{{ t("resource.depGraphReset") }}</BaseButton>
      <BaseButton variant="accent" @click="emit('close')">{{ t("actions.confirm") }}</BaseButton>
    </div>
  </BaseModal>
</template>

<style scoped>
/* 画布吃掉剩余高度；vue-flow 自己铺满它 */
.dep-flow {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--bg-raised);
}

/* 选中的那条线加粗走主色：默认样式太细，点中了也看不出来 */
.dep-flow :deep(.vue-flow__edge.selected .vue-flow__edge-path) {
  stroke: var(--accent);
  stroke-width: 2.5;
}

/* 选中的节点：边框**直接换成主色**，不叠第二层。
   主色就是本应用"选中"的颜色（资源列表的选中行也用它）；"当前模组"（图的根）
   因此改成了浅主色底纹，不再占边框色。主题自带的 .selected 样式只认它内置的
   default / input / output 三种节点，自定义节点得自己给。
   代价是选中时看不出"必装 / 可选 / 游戏加载器"，选中态优先 */
.dep-flow :deep(.vue-flow__node.selected .dep-node) {
  border-color: var(--accent);
}

/* 贴在鼠标旁边的依赖说明：fixed 定位，跟着视口走。
   pointer-events: none —— 它压着画布，不穿透的话下一次点空白收不起来 */
.dep-edge-popup {
  position: fixed;
  z-index: 20;
  max-width: 420px;
  margin: 0;
  padding: 6px 10px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg-solid, var(--bg-card));
  color: var(--text);
  font-size: 12px;
  line-height: 1.5;
  box-shadow: var(--shadow-lg);
  pointer-events: none;
  word-break: break-all;
}
</style>
