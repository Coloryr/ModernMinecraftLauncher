/**
 * 文件树里某条路径最终算不算勾选
 *
 * 勾选沿目录向下继承：勾了目录，里面每一条都算勾上，树上也就都显示成勾选。
 * 同一个路径上最深的一次操作说话——`.minecraft` 勾着、`logs` 取消，`logs` 就是不勾。
 *
 * - `path`: 相对文件树根目录的路径
 * - `picked`: 勾上的条目表，键是路径
 * - `unpicked`: 取消勾选的路径表
 *
 * # 返回值
 *
 * 是否勾选
 */
export function isPickedPath(
  path: string,
  picked: ReadonlyMap<string, unknown>,
  unpicked: ReadonlySet<string>,
): boolean {
  let current = path;
  for (; ;) {
    if (unpicked.has(current)) return false;
    if (picked.has(current)) return true;
    const cut = current.lastIndexOf("/");
    if (cut < 0) return false;
    current = current.slice(0, cut);
  }
}
