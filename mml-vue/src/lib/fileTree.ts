// 文件树：把扁平路径列表解析成树，并提供收集工具
// 目录以 "/" 结尾，如 "minecraft/mods/"，其余为文件。

export interface FileNode {
  /** 完整路径：规范 key 一律不含尾部斜杠，是否目录看 isDir */
  key: string;
  name: string;
  isDir: boolean;
  children: FileNode[];
  /** 目录子节点尚未加载：懒加载时首次展开会通过事件加载 */
  lazy?: boolean;
}

/**
 * 把扁平路径列表解析为树
 *
 * - `paths`: 扁平路径列表，目录以 `/` 结尾
 *
 * # 返回值
 *
 * 根节点列表，路径里的公共前缀已合并成目录节点
 */
export function buildTree(paths: string[]): FileNode[] {
  const root: FileNode[] = [];
  const map = new Map<string, FileNode>();
  for (const p of paths) {
    const isDirEntry = p.endsWith("/");
    const parts = p.split("/").filter(Boolean);
    let cur: FileNode[] = root;
    let acc = "";
    for (let i = 0; i < parts.length; i++) {
      acc = acc ? `${acc}/${parts[i]}` : parts[i];
      const isLast = i === parts.length - 1;
      let node = map.get(acc);
      if (!node) {
        node = { key: acc, name: parts[i], isDir: !isLast || isDirEntry, children: [] };
        map.set(acc, node);
        cur.push(node);
      }
      cur = node.children;
    }
  }
  return root;
}

/**
 * 收集所有文件叶子的 key
 *
 * - `nodes`: 待遍历的节点
 * - `acc`: 结果累加数组，调用方一般不传
 *
 * # 返回值
 *
 * 全部文件 key
 */
export function collectFileKeys(nodes: FileNode[], acc: string[] = []): string[] {
  for (const n of nodes) {
    if (n.isDir) collectFileKeys(n.children, acc);
    else acc.push(n.key);
  }
  return acc;
}

/**
 * 收集所有目录 key
 *
 * - `nodes`: 待遍历的节点
 * - `acc`: 结果累加数组，调用方一般不传
 *
 * # 返回值
 *
 * 全部目录 key
 */
export function collectDirKeys(nodes: FileNode[], acc: string[] = []): string[] {
  for (const n of nodes) {
    if (n.isDir) {
      acc.push(n.key);
      collectDirKeys(n.children, acc);
    }
  }
  return acc;
}
