// 收藏卡片的自定义拖拽：把收藏项拖到某个分组上 = 移到那个分组
//
// 与实例拖拽 `useInstanceDrag` 同一套路数：**不用** HTML5 draggable ——
// WebView2 里会出现"禁止"光标且投放不可靠 —— 改用指针事件模拟：
// 按下记候选 → 移动超阈值开始拖 → 指针下方找分组 → 松开提交。
//
// 两者刻意没有合并成一个通用件：**落点语义不同**。实例拖拽要算"组内第几行"，
// 即插入位置 —— 内核分组表存了组内次序，还要画插入占位；收藏这边分组在数据层就是个
// 集合 `HashSet<uuid>`，DTO 里按 uuid 排序下发，只表达"归到哪个组"，没有次序可算，
// 所以这边只需要"指针落在哪个组上"，提交也只是一次 `collect_set_group_items`。
import { computed, onMounted, onUnmounted, ref, type Ref } from "vue";
import { api } from "../lib/api";
import { tErr } from "../lib/i18n";
import { showToast } from "../lib/toast";
import type { CollectItemDto } from "../lib/bindings";
import { elementBeneathShield, useDragGhost } from "./useDragGhost";

interface DragDeps {
  /** 当前勾选的收藏项 */
  checked: Ref<Set<string>>;
  /** 多选模式是否开着 */
  multiSelect: Ref<boolean>;
  /** uuid → 它所在的具名分组名（未归组没有条目） */
  groupOf: () => Map<string, string>;
  /** 「默认分组」的键；拖到这里 = 移出所有具名分组 */
  defaultGroup: string;
}

/** 一次拖拽要移动的收藏项 */
interface DragCandidate {
  /** 目标 uuid：多选模式下拖一张已勾选的卡片时是一批，否则只有它自己 */
  uuids: string[];
}

/** 开始拖拽的位移阈值（与实例拖拽一致） */
const DRAG_THRESHOLD = 5;
/** 靠近内容区上下边缘多少像素时自动滚动 */
const SCROLL_EDGE = 60;
/** 自动滚动每次挪多少像素 */
const SCROLL_STEP = 10;

/**
 * 收藏卡片的自定义拖拽
 *
 * - `deps`: 勾选集合、多选开关与分组归属查询，均由调用方持有
 *
 * # 返回值
 *
 * - `dragActive`: 正在拖的批次，`null` 表示没在拖
 * - `ghost`: 跟手浮起副本（批次 + 指针位置），`null` 表示没在拖
 * - `draggingUuids`: 正在拖的项 uuid 集合，卡片据此变淡
 * - `dropGroup`: 指针当前落在的分组 key，`null` 表示不在任何分组上
 * - `onCardPointerDown`: 卡片按下时调用，记下拖拽候选
 * - `consumeSuppressClick`: 取走"抑制紧随 click"的标记，返回 `true` 表示本次 click 该忽略
 */
export function useCollectDrag(deps: DragDeps) {
  const dragCandidate = ref<DragCandidate | null>(null);
  const dragActive = ref<DragCandidate | null>(null);
  /** 指针当前落在哪个分组上；键是分组的 key，`null` 表示不在任何分组上，松手即取消 */
  const dropGroup = ref<string | null>(null);
  /** 正在拖拽的收藏项（卡片据此变淡） */
  const draggingUuids = computed(() => new Set(dragActive.value?.uuids ?? []));
  /** 跟手浮起副本：内容由调用方按 `ghost.item` 的 uuid 现查 */
  const dragGhost = useDragGhost<DragCandidate>();

  let pointerId: number | null = null;
  let startX = 0;
  let startY = 0;
  /** 拖拽结束后抑制紧随的 click；否则一松手就顺手打开了下载窗口 */
  let suppressClick = false;

  /** 卡片按下：记候选（真正开始拖要等移动超过阈值） */
  function onCardPointerDown(e: PointerEvent, item: CollectItemDto) {
    if (e.button !== 0) return;
    suppressClick = false;
    pointerId = e.pointerId;
    startX = e.clientX;
    startY = e.clientY;
    // 多选模式下拖一张**已勾选**的卡片：整批一起走（与「移动到分组」弹窗同一语义）；
    // 其余情况只拖按下的这一张。
    // 按下那一张排最前：拖拽幽灵卡片显示的是"第一项"，该是用户手里抓着的那张
    const picked = deps.checked.value;
    const uuids =
      deps.multiSelect.value && picked.has(item.uuid)
        ? [item.uuid, ...[...picked].filter((uuid) => uuid !== item.uuid)]
        : [item.uuid];
    dragCandidate.value = { uuids };
  }

  function onPointerMove(e: PointerEvent) {
    if (pointerId === null || e.pointerId !== pointerId) return;
    if (!dragActive.value) {
      // 还没到阈值：按下可能只是想点卡片 / 点勾选框，别急着当拖拽
      if (Math.hypot(e.clientX - startX, e.clientY - startY) <= DRAG_THRESHOLD) return;
      dragActive.value = dragCandidate.value;
      if (!dragActive.value) return;
      dragGhost.startGhost(dragActive.value, e);
      // 真拖起来了：这一串指针操作结束时的 click 不算"点卡片"，见 consumeSuppressClick
      suppressClick = true;
    } else {
      dragGhost.moveGhost(e);
    }
    updateDropGroup(e);
  }

  function onPointerUp(e: PointerEvent) {
    if (pointerId === null || e.pointerId !== pointerId) return;
    const active = dragActive.value;
    const target = dropGroup.value;
    resetDrag();
    if (active && target !== null) {
      void commitDrag(active, target);
    }
  }

  function resetDrag() {
    pointerId = null;
    dragCandidate.value = null;
    dragActive.value = null;
    dropGroup.value = null;
    dragGhost.stopGhost();
  }

  /** 指针下方是哪个分组；用 DOM 反向查找，`data-group` 存分组 key（左栏那一项），与实例拖拽同一做法 */
  function updateDropGroup(e: PointerEvent) {
    const el = elementBeneathShield(e.clientX, e.clientY);

    // 靠近内容区上下边缘自动滚动，方便投给远处的分组（分组多时不然够不到）
    const body = (el?.closest?.(".frame-body") ?? null) as HTMLElement | null;
    if (body) {
      const rect = body.getBoundingClientRect();
      if (e.clientY < rect.top + SCROLL_EDGE) body.scrollTop -= SCROLL_STEP;
      else if (e.clientY > rect.bottom - SCROLL_EDGE) body.scrollTop += SCROLL_STEP;
    }

    const block = (el?.closest?.("[data-group]") ?? null) as HTMLElement | null;
    // 挂上了 data-group 就一定取得到 key；
    // 没挂在任何分组上记 null —— 「全部」那一项没有 data-group
    dropGroup.value = block ? (block.dataset.group ?? "") : null;
  }

  /** 提交：把这次拖拽的项移到落点分组 */
  async function commitDrag(active: DragCandidate, target: string) {
    // 已经在目标组里的不用动：拖回原组就是一次空操作，别白写一次盘、白广播一次事件
    const uuids = active.uuids.filter(
      (uuid) => (deps.groupOf().get(uuid) ?? deps.defaultGroup) !== target,
    );
    if (!uuids.length) return;

    try {
      if (target === deps.defaultGroup) {
        // 默认分组不是具名分组：把这些项从各自所在的组里摘出来，摘完自然回到默认分组
        const byGroup = new Map<string, string[]>();
        for (const uuid of uuids) {
          const name = deps.groupOf().get(uuid);
          if (!name) continue;
          const list = byGroup.get(name) ?? [];
          list.push(uuid);
          byGroup.set(name, list);
        }
        for (const [name, list] of byGroup) {
          await api.collectRemoveItems(list, name);
        }
      } else {
        // 移动语义：后端会先把它们从其它分组里摘掉，见 collect_utils::set_group_items
        await api.collectSetGroupItems(target, uuids);
      }
      // 移走的项已经不在原来的组里了，勾选跟着清掉（与「移动到分组」弹窗一致）
      deps.checked.value = new Set();
    } catch (e) {
      showToast(tErr(e));
    }
  }

  /** 消费拖拽后的抑制点击标记；返回 true 表示本次 click 应被忽略 */
  function consumeSuppressClick(): boolean {
    if (suppressClick) {
      suppressClick = false;
      return true;
    }
    return false;
  }

  onMounted(() => {
    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
    // 指针被系统收走（触摸被打断等）时当作取消：不提交，也别把卡片留在"拖拽中"的状态
    window.addEventListener("pointercancel", resetDrag);
  });
  onUnmounted(() => {
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("pointerup", onPointerUp);
    window.removeEventListener("pointercancel", resetDrag);
  });

  // 拖拽一旦开始，紧随的 click 就不该再当成"点卡片"
  return {
    dragActive,
    ghost: dragGhost.ghost,
    draggingUuids,
    dropGroup,
    onCardPointerDown,
    consumeSuppressClick,
  };
}
