// 多选模式逻辑：右键分组全选进入多选、勾选切换、强制展开分组并居中滚动
import { nextTick, ref, type Ref } from "vue";
import type { InstanceInfoDto } from "../lib/bindings";

interface MultiDeps {
  /** 当前单选中的实例；进入多选时清空 */
  selected: Ref<InstanceInfoDto | null>;
  /** 新闻页是否打开；进入多选时关掉 */
  newsActive: Ref<boolean>;
  /** 分组折叠表；进入多选时全部展开 */
  collapsedGroups: Ref<Record<string, boolean>>;
  /** 退出多选时回调（例如关闭右键菜单） */
  onExit?: () => void;
}

/**
 * 多选模式：右键分组全选进入、勾选切换、强制展开分组并居中滚动
 *
 * - `deps`: 当前选中实例、新闻页开关、分组折叠表与退出回调，均由调用方持有
 *
 * # 返回值
 *
 * - `multiSelect`: 多选模式是否开着
 * - `selectedIds`: 已勾选的实例 uuid 集合
 * - `enterMultiSelect`: 进入多选并勾选给定的一批 uuid
 * - `exitMultiSelect`: 退出多选并清空勾选
 * - `toggleSelect`: 切换单个实例的勾选；勾掉最后一个会退出多选
 */
export function useMultiSelect(deps: MultiDeps) {
  const multiSelect = ref(false);
  const selectedIds = ref<Set<string>>(new Set());

  /** 把第一个选中实例滚动到可视区域正中 */
  function scrollFirstSelectedIntoView() {
    const first = [...selectedIds.value][0];
    if (!first) return;
    const row = document.querySelector<HTMLElement>(`.inst-row[data-uuid="${first}"]`);
    row?.scrollIntoView({ block: "center", behavior: "smooth" });
  }

  /**
   * 进入多选：勾选给定的一批实例，并清空单选与新闻页
   *
   * - `ids`: 要勾选的实例 uuid
   */
  function enterMultiSelect(ids: string[]) {
    multiSelect.value = true;
    deps.selected.value = null;
    deps.newsActive.value = false;
    selectedIds.value = new Set(ids);
    // 勾选的行可能藏在折叠的分组里，全部展开才看得见
    deps.collapsedGroups.value = {};
    nextTick(scrollFirstSelectedIntoView);
  }

  /** 退出多选：清空勾选并通知调用方（例如收起右键菜单） */
  function exitMultiSelect() {
    multiSelect.value = false;
    selectedIds.value = new Set();
    deps.onExit?.();
  }

  /**
   * 切换单个实例的勾选
   *
   * - `inst`: 被点击的实例
   */
  function toggleSelect(inst: InstanceInfoDto) {
    const s = new Set(selectedIds.value);
    const adding = !s.has(inst.uuid);
    if (adding) s.add(inst.uuid);
    else s.delete(inst.uuid);
    selectedIds.value = s;
    if (s.size === 0) exitMultiSelect();
    else if (adding) nextTick(scrollFirstSelectedIntoView);
  }

  return { multiSelect, selectedIds, enterMultiSelect, exitMultiSelect, toggleSelect };
}
