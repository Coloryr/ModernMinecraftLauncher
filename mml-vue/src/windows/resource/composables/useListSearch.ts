// 资源列表的文本搜索：按一行拼出来的文本匹配，关键词留在这个组合式函数里
//
// 各分类的搜索框都挂在各自的顶栏（.head-search-wrap），过滤只作用于**看得见的列表**：
// 数据本身、批量操作的目标、云同步那类按全量算的东西都不动。
import { computed, ref } from "vue";

/**
 * 给一组行挂上文本搜索
 *
 * - `items`: 当前列表
 * - `textOf`: 取一行的可搜索文本，多段用 `\n` 连；`§` 格式码由调用方先去掉
 *
 * # 返回值
 *
 * - `keyword`: 搜索词，绑定 SearchBox
 * - `searching`: 是否正在过滤
 * - `filtered`: 过滤后的列表；没搜索时就是原列表
 */
export function useListSearch<T>(items: () => T[], textOf: (item: T) => string) {
  const keyword = ref("");
  const query = computed(() => keyword.value.trim().toLowerCase());

  const filtered = computed(() => {
    const q = query.value;
    const list = items();
    if (!q) return list;
    return list.filter((item) => textOf(item).toLowerCase().includes(q));
  });

  return { keyword, searching: computed(() => !!query.value), filtered };
}
