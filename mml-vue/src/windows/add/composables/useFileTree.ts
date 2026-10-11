// 添加实例 · 文件树状态：压缩包模式与文件夹模式各持一份实例
//
// 两棵树的操作完全同构，勾选 / 展开 / 全选 / 目录级联都一样，差别只在「怎么把节点读进来」，
// 所以这里只管状态与操作，读盘与懒加载留在窗口里。
import { ref } from "vue";
import { collectFileKeys, type FileNode } from "../../../lib/fileTree";

/**
 * 文件树状态：勾选 / 展开 / 全选 / 目录级联
 *
 * # 返回值
 *
 * - `tree`: 根节点列表，懒加载时子节点随展开补齐
 * - `checked`: 已勾选的叶子文件 key
 * - `expanded`: 已展开的目录 key
 * - `reset` / `setAll` / `toggleFile` / `toggleDir` / `toggleExpand`: 状态操作方法
 * - `unselectedKeys`: 未勾选的文件 key，调用方据此决定要不要传排除名单
 */
export function useFileTree() {
  /** 根节点列表（懒加载时子节点随展开补齐） */
  const tree = ref<FileNode[]>([]);
  /** 已勾选的文件 key（只含叶子文件） */
  const checked = ref<Set<string>>(new Set());
  /** 已展开的目录 key */
  const expanded = ref<Set<string>>(new Set());

  /** 用新节点替换整棵树；`checkedAll` 决定默认是否全选，展开态一律复位 */
  function reset(nodes: FileNode[] = [], checkedAll = false) {
    tree.value = nodes;
    checked.value = checkedAll ? new Set(collectFileKeys(nodes)) : new Set();
    expanded.value = new Set();
  }

  /** 全选 / 全不选；只针对已加载的节点，懒加载目录展开后需再点一次 */
  function setAll(on: boolean) {
    checked.value = on ? new Set(collectFileKeys(tree.value)) : new Set();
  }

  function toggleFile(key: string) {
    const s = new Set(checked.value);
    if (s.has(key)) s.delete(key);
    else s.add(key);
    checked.value = s;
  }

  /** 目录勾选：级联到其下所有文件 */
  function toggleDir(node: FileNode, on: boolean) {
    const s = new Set(checked.value);
    for (const f of collectFileKeys(node.children)) {
      if (on) s.add(f);
      else s.delete(f);
    }
    checked.value = s;
  }

  function toggleExpand(key: string) {
    const s = new Set(expanded.value);
    if (s.has(key)) s.delete(key);
    else s.add(key);
    expanded.value = s;
  }

  /** 未勾选的文件 key：全选时为空，调用方据此决定要不要传排除名单 */
  function unselectedKeys(): string[] {
    return collectFileKeys(tree.value).filter((k) => !checked.value.has(k));
  }

  return { tree, checked, expanded, reset, setAll, toggleFile, toggleDir, toggleExpand, unselectedKeys };
}
