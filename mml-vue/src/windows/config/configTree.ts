// 配置文件树：按来源内的路径建树，再按展开状态与搜索词算可见行
//
// 树只建一次，展开与搜索都只影响可见行，不动树本身。
import { buildTree, type FileNode } from "../../lib/fileTree.ts";

/** 配置文件树中当前可见的一行 */
export interface ConfigTreeRow {
  /** 来源内的完整路径，目录不带末尾斜杠 */
  key: string;
  /** 路径最后一段，用于显示 */
  name: string;
  /** 是否为目录 */
  isDir: boolean;
  /** 缩进层级，从 0 开始 */
  depth: number;
  /** 目录是否展开，包含搜索时的自动展开状态 */
  expanded: boolean;
}

/** 每层目录在前、文件在后，同类节点按名称排序 */
function sortDirsFirst(nodes: FileNode[]) {
  nodes.sort((a, b) => (a.isDir === b.isDir ? a.name.localeCompare(b.name) : a.isDir ? -1 : 1));
  for (const node of nodes) {
    if (node.isDir) sortDirsFirst(node.children);
  }
}

/**
 * 从来源内的路径构建有序文件树
 *
 * - `paths`: 相对路径，目录以 `/` 结尾
 *
 * # 返回值
 *
 * 每层均按目录优先排列的根节点列表
 */
export function buildConfigTree(paths: string[]): FileNode[] {
  const nodes = buildTree(paths);
  sortDirsFirst(nodes);
  return nodes;
}

/**
 * 按展开状态和搜索词生成文件树的可见行
 *
 * - `nodes`: 已排序的文件树
 * - `expanded`: 用户展开的目录路径，搜索不会修改此集合
 * - `query`: 按完整路径匹配，忽略大小写和首尾空白
 *
 * # 返回值
 *
 * 可见行；搜索时保留命中项的祖先目录，并自动展开
 */
export function visibleConfigRows(
  nodes: FileNode[],
  expanded: ReadonlySet<string>,
  query: string,
): ConfigTreeRow[] {
  const q = query.trim().toLowerCase();
  const rows: ConfigTreeRow[] = [];
  const walk = (branch: FileNode[], depth: number) => {
    for (const node of branch) {
      const start = rows.length;
      const opened = node.isDir && (!!q || expanded.has(node.key));
      rows.push({ key: node.key, name: node.name, isDir: node.isDir, depth, expanded: opened });
      if (opened) walk(node.children, depth + 1);
      // 子项遍历后再剔除空分支，避免为每个祖先重复扫描同一子树。
      if (q && rows.length === start + 1 && !node.key.toLowerCase().includes(q)) rows.pop();
    }
  };
  walk(nodes, 0);
  return rows;
}
