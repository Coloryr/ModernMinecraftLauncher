// 自定义拖拽逻辑：实例排序 / 跨组移动 / 分组排序
// 不用 HTML5 draggable —— WebView2 中会出现“禁止”光标且投放不可靠 ——
// 改为指针事件模拟：按下记录候选 → 移动超阈值开始拖拽 →
// 组内插入占位（淡化的实例）→ 松开提交；支持同组排序与跨组移动。
import { computed, onMounted, onUnmounted, ref, type Ref } from "vue";
import { api } from "../lib/api";
import type { InstanceInfoDto } from "../lib/bindings";
import { elementBeneathShield, useDragGhost } from "./useDragGhost";

export interface GroupView {
  /** 分组 uuid，作为身份 */
  id: string;
  /** 分组显示名 */
  name: string;
  /** 是否默认分组 */
  isDefault: boolean;
  /** 组内实例，按显示顺序 */
  items: InstanceInfoDto[];
}

interface DragDeps {
  /** 多选模式是否开着；开着时不拖 */
  multiSelect: Ref<boolean>;
  /** 分组视图，落点与插入占位都按它算 */
  groups: Ref<GroupView[]>;
  /** 分组折叠表；跨组投放后展开目标分组 */
  collapsedGroups: Ref<Record<string, boolean>>;
  /** 重拉实例列表 */
  loadInstances: () => Promise<void> | void;
  /** 重拉分组列表 */
  loadGroups: () => Promise<void> | void;
}

interface DragCandidate {
  /** 拖的是实例还是分组 */
  kind: "instance" | "group";
  /** 被拖的实例；kind 为 instance 时有值 */
  instance?: InstanceInfoDto;
  /** 分组 uuid，拖动与落点判断都用它 */
  groupId?: string;
  /** 分组显示名，只用于拖拽占位上的文案 */
  groupName?: string;
}

/**
 * 实例排序 / 跨组移动 / 分组排序的自定义拖拽
 *
 * - `deps`: 多选开关、分组视图、折叠表与两个重拉回调，均由调用方持有
 *
 * # 返回值
 *
 * - `dragActive`: 正在拖的候选，`null` 表示没在拖
 * - `ghost`: 跟手浮起副本（候选 + 指针位置），`null` 表示没在拖
 * - `draggingUuid`: 正在拖的实例 uuid
 * - `dragInsert`: 实例的插入位置，分组 uuid + 组内下标
 * - `dragGroupInsert`: 分组的插入位置
 * - `onDragPointerDown`: 实例行 / 分组按下时调用，记下拖拽候选
 * - `isInstInsert`: 某组第 idx 个实例之前是否该画插入占位
 * - `isInstInsertEnd`: 某组末尾是否该画插入占位
 * - `isGroupInsert`: 分组列表第 index 个之前是否该画插入占位
 * - `consumeSuppressClick`: 取走"抑制紧随 click"的标记，返回 `true` 表示本次 click 该忽略
 */
export function useInstanceDrag(deps: DragDeps) {
  const dragCandidate = ref<DragCandidate | null>(null);
  const dragActive = ref<DragCandidate | null>(null);
  /** 正在拖拽的实例 uuid；拖拽时该行不渲染，只显示插入占位 */
  const draggingUuid = computed(() =>
    dragActive.value?.kind === "instance"
      ? (dragActive.value.instance?.uuid ?? null)
      : null,
  );
  /** 实例拖拽插入位置（分组 uuid + 组内下标） */
  const dragInsert = ref<{ groupId: string; index: number } | null>(null);
  /** 分组拖拽插入位置 */
  const dragGroupInsert = ref<{ index: number } | null>(null);
  /** 跟手浮起副本：内容由调用方按 `ghost.item` 渲染 */
  const dragGhost = useDragGhost<DragCandidate>();

  let dragPointerId: number | null = null;
  let dragStartX = 0;
  let dragStartY = 0;
  /** 拖拽结束后抑制紧随的 click（避免误触选择 / 折叠） */
  let suppressClick = false;

  /**
   * 记下拖拽候选；多选模式下与非左键都不拖
   *
   * - `e`: 指针按下事件
   * - `cand`: 被按住的实例或分组
   */
  function onDragPointerDown(e: PointerEvent, cand: DragCandidate) {
    if (e.button !== 0 || deps.multiSelect.value) return;
    suppressClick = false;
    dragPointerId = e.pointerId;
    dragStartX = e.clientX;
    dragStartY = e.clientY;
    dragCandidate.value = cand;
  }

  function onDragPointerMove(e: PointerEvent) {
    if (dragPointerId === null || e.pointerId !== dragPointerId) return;
    if (!dragActive.value) {
      const d = Math.hypot(e.clientX - dragStartX, e.clientY - dragStartY);
      if (d <= 5) return;
      const cand = dragCandidate.value;
      if (!cand) return;
      dragActive.value = cand;
      dragGhost.startGhost(cand, e);
      suppressClick = true;
      document.body.classList.add("drag-active");
    }
    dragGhost.moveGhost(e);
    updateDragInsert(e);
  }

  function onDragPointerUp(e: PointerEvent) {
    // 只认左键松开：拖到一半按右键再松开，不该把这次拖拽提交掉
    if (dragPointerId === null || e.pointerId !== dragPointerId || e.button !== 0) return;
    if (dragActive.value) commitDrag(dragActive.value);
    resetDrag();
  }

  /** 清掉本次拖拽的全部状态与 body 上的拖拽标记 */
  function resetDrag() {
    dragPointerId = null;
    dragCandidate.value = null;
    dragActive.value = null;
    dragInsert.value = null;
    dragGroupInsert.value = null;
    dragGhost.stopGhost();
    document.body.classList.remove("drag-active");
  }

  /** 按指针位置算插入占位 */
  function updateDragInsert(e: PointerEvent) {
    const active = dragActive.value;
    if (!active) return;
    const el = elementBeneathShield(e.clientX, e.clientY);
    // 靠近分组列表边缘自动滚动，方便投放到远处的分组
    const listEl = el?.closest?.(".group-list") as HTMLElement | null;
    if (listEl) {
      const rect = listEl.getBoundingClientRect();
      // 边缘热区，单位 px
      const threshold = 40;
      if (e.clientY < rect.top + threshold) listEl.scrollTop -= 10;
      else if (e.clientY > rect.bottom - threshold) listEl.scrollTop += 10;
    }
    if (active.kind === "instance") {
      // 行是虚拟滚动的，只渲染视口附近那几行：落点按"压在哪一行、行上半还是下半"加该行的
      // data-idx 算，不能数渲染出来的行数（被藏起来的行没有 DOM）
      const groupEl = (el?.closest?.("[data-group]") ?? null) as HTMLElement | null;
      if (!groupEl) {
        dragInsert.value = null;
        return;
      }
      // data-group 存的是分组 uuid（身份；组名可改）；默认分组也有 uuid，只有拿不到分组表时才是空串
      const groupId = groupEl.dataset.group ?? "";
      const rows = listEl
        ? ([...listEl.querySelectorAll(".inst-row")] as HTMLElement[]).filter(
          (r) => r.dataset.group === groupId,
        )
        : [];
      let index = 0;
      for (const row of rows) {
        const rect = row.getBoundingClientRect();
        const at = Number(row.dataset.idx ?? 0);
        if (e.clientY < rect.top + rect.height / 2) {
          index = at;
          break;
        }
        index = at + 1;
      }
      // data-idx 是组内完整下标（含正在拖的那个），接口要的是"拿掉它之后"的下标
      const dragged =
        deps.groups.value
          .find((g) => g.id === groupId)
          ?.items.findIndex((i) => i.uuid === active.instance?.uuid) ?? -1;
      if (dragged >= 0 && dragged < index) index -= 1;
      dragInsert.value = { groupId, index };
    } else {
      // 分组标题行带 data-gi，同样按它算插到第几个分组之前
      const headers = listEl ? ([...listEl.querySelectorAll("[data-gi]")] as HTMLElement[]) : [];
      let index = headers.length
        ? Number(headers[headers.length - 1].dataset.gi ?? 0) + 1
        : 0;
      for (const header of headers) {
        const rect = header.getBoundingClientRect();
        if (e.clientY < rect.top + rect.height / 2) {
          index = Number(header.dataset.gi ?? 0);
          break;
        }
      }
      dragGroupInsert.value = { index };
    }
  }

  /** 松手提交：实例走 moveInstance，分组走 moveGroup，成功后重拉对应列表 */
  async function commitDrag(active: DragCandidate) {
    if (active.kind === "instance" && active.instance) {
      const ins = dragInsert.value;
      if (!ins) return;
      // 落点由后端记进内核分组表（归属 + 组内次序），重拉列表即拿到新次序。
      // 这里直接给分组 uuid：默认分组也有自己的 uuid，与 null 等价
      await api.moveInstance(active.instance.uuid, ins.groupId, ins.index);
      await deps.loadInstances();
      // 展开目标分组
      deps.collapsedGroups.value = { ...deps.collapsedGroups.value, [ins.groupId]: false };
    } else if (active.kind === "group" && active.groupId) {
      const ins = dragGroupInsert.value;
      if (!ins) return;
      await api.moveGroup(active.groupId, ins.index);
      await deps.loadGroups();
    }
  }

  /** 实例插入占位：分组 groupId 中第 idx 个实例之前；末尾由 isInstInsertEnd 渲染，避免同组拖动出现双占位 */
  function isInstInsert(groupId: string, idx: number): boolean {
    const ins = dragInsert.value;
    if (!ins || ins.groupId !== groupId) return false;
    const active = dragActive.value;
    if (active?.kind !== "instance") return false;
    const dragging = active.instance;
    if (!dragging) return false;
    const items = deps.groups.value.find((g) => g.id === groupId)?.items ?? [];
    // 组内非拖拽实例总数
    const total = items.filter((i) => i.uuid !== dragging.uuid).length;
    let before = 0;
    for (let i = 0; i < idx; i++) {
      if (items[i].uuid !== dragging.uuid) before++;
    }
    return before === ins.index && ins.index < total;
  }

  /** 实例插入占位：分组末尾 */
  function isInstInsertEnd(groupId: string): boolean {
    const ins = dragInsert.value;
    if (!ins || ins.groupId !== groupId) return false;
    const active = dragActive.value;
    if (active?.kind !== "instance") return false;
    const dragging = active.instance;
    if (!dragging) return false;
    const items = deps.groups.value.find((g) => g.id === groupId)?.items ?? [];
    const before = items.filter((i) => i.uuid !== dragging.uuid).length;
    return before === ins.index;
  }

  /** 分组插入占位：分组列表第 index 个之前 */
  function isGroupInsert(index: number): boolean {
    return (
      dragActive.value?.kind === "group" &&
      !!dragGroupInsert.value &&
      dragGroupInsert.value.index === index
    );
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
    window.addEventListener("pointermove", onDragPointerMove);
    window.addEventListener("pointerup", onDragPointerUp);
    // 指针被系统收走（触摸被打断等）时当作取消，别把占位留成"一直在拖"
    window.addEventListener("pointercancel", resetDrag);
  });
  onUnmounted(() => {
    window.removeEventListener("pointermove", onDragPointerMove);
    window.removeEventListener("pointerup", onDragPointerUp);
    window.removeEventListener("pointercancel", resetDrag);
    // 拖到一半窗口被关掉：body 上的 .drag-active 只有这里能摘
    resetDrag();
  });

  return {
    dragActive,
    ghost: dragGhost.ghost,
    draggingUuid,
    dragInsert,
    dragGroupInsert,
    onDragPointerDown,
    isInstInsert,
    isInstInsertEnd,
    isGroupInsert,
    consumeSuppressClick,
  };
}
