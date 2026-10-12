// 资源列表行的多选：勾选框加选、Shift 整段选择、锚点行
//
// **各分类共用这一套**（模组、存档、数据包、材质包、光影、结构、截图、服务器），口径只留一份：
// - 选中身份是**字符串键**，由调用方给（模组用 SHA1 / uuid，存档用目录名，其余用文件名 /
//   登记名 / 名字 + 地址）；键在批量操作前后必须稳定，否则操作做完选中就散了
// - 整段选择按**当前看得见的显示顺序**，由调用方的 `order` 给：收起的分组看不见，
//   不算进整段 —— 选中的就是屏幕上那一段
//
// 组件侧只负责画：勾选框走 ResourceRow 的 `select` 插槽（截图那个自绘网格浮在格子角上），
// 选中 / 锚点行挂类名，样式在窗口级 resource.css 里（非 scoped，各分类共用）。
import { computed, ref } from "vue";

/**
 * 给一组行挂上多选
 *
 * - `options.keyOf`: 取某一行的选中键；返回空串表示这一行不参与多选
 * - `options.order`: Shift 整段选择用的顺序，即当前**看得见**的行按显示顺序排好
 *
 * # 返回值
 *
 * - `selected`: 已选中的键集合，行组件据此判断选中态
 * - `anchorKey`: 锚点行的键，即整段选择的起点；没点过任何一行时是 `null`
 * - `count`: 选中数量
 * - `isSelected`: 某一行是否选中
 * - `toggle`: 勾选框用，加选 / 移出，并把它记成锚点
 * - `onRowClick`: 整行左键用，Shift 是整段选中，否则只在多选状态下加选 / 反选
 * - `clear`: 清空选择与锚点
 */
export function useRowSelect<T>(options: {
  keyOf: (item: T) => string;
  order: () => T[];
}) {
  const selected = ref<Set<string>>(new Set());

  /** Shift 整段选择的锚点：上次点或勾选的那一项 */
  const anchorKey = ref<string | null>(null);

  const count = computed(() => selected.value.size);

  function isSelected(item: T): boolean {
    return selected.value.has(options.keyOf(item));
  }

  /** 把这一行加进 / 移出选择 */
  function toggle(item: T) {
    const key = options.keyOf(item);
    if (!key) return;
    const next = new Set(selected.value);
    if (next.has(key)) {
      next.delete(key);
    } else {
      next.add(key);
    }
    selected.value = next;
    // 记住"上次点（或勾选）的那一项"：Shift + 点的整段选择从这里算起
    anchorKey.value = key;
  }

  /** 清空选择与锚点 */
  function clear() {
    selected.value = new Set();
    // 锚点也一起清：取消选择后不该还留着"整段从这里开始"的标记
    anchorKey.value = null;
  }

  /**
   * Shift + 点：整段选中，**替换**当前选择
   *
   * 没有锚点时，也就是第一次就是 Shift + 点，只选这一项。
   * **锚点不动**：它始终是最初点的那一项，反复 Shift + 点都从同一个起点重新划范围。
   */
  function selectRange(item: T) {
    const order = options.order();
    const key = options.keyOf(item);
    const to = order.findIndex((x) => options.keyOf(x) === key);
    if (to < 0) return;

    const from = order.findIndex((x) => options.keyOf(x) === anchorKey.value);
    if (from < 0) {
      selected.value = new Set([key]);
      anchorKey.value = key;
      return;
    }

    const [lo, hi] = from <= to ? [from, to] : [to, from];
    selected.value = new Set(
      order
        .slice(lo, hi + 1)
        .map((x) => options.keyOf(x))
        .filter((each) => !!each),
    );
  }

  /**
   * 整行左键
   *
   * 普通点击也**记下锚点**：这样"先点一行、再 Shift + 点另一行"就能选出这一段，
   * 即使第一次点击本身没有改变选择 —— 不在多选状态时它什么都不做。
   */
  function onRowClick(payload: { event: MouseEvent; item: T }) {
    if (payload.event.shiftKey) {
      selectRange(payload.item);
      return;
    }
    anchorKey.value = options.keyOf(payload.item);
    if (!selected.value.size) return;
    toggle(payload.item);
  }

  return { selected, anchorKey, count, isSelected, toggle, onRowClick, clear };
}
