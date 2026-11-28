import type { ExportEntryDto } from "../../lib/bindings";

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

export type FileCheckState = "checked" | "unchecked" | "mixed";

function parentPaths(paths: Iterable<string>, ignored?: ReadonlySet<string>): Set<string> {
  const parents = new Set<string>();
  for (let path of paths) {
    if (ignored?.has(path)) continue;
    for (let end = path.lastIndexOf("/"); end >= 0; end = path.lastIndexOf("/")) {
      path = path.slice(0, end);
      if (parents.has(path)) break;
      parents.add(path);
    }
  }
  return parents;
}

export function fileCheckStates(
  children: ReadonlyMap<string, readonly ExportEntryDto[]>,
  picked: ReadonlyMap<string, unknown>,
  unpicked: ReadonlySet<string>,
): Map<string, FileCheckState> {
  const withPicked = parentPaths(picked.keys(), unpicked);
  const withUnpicked = parentPaths(unpicked);
  const states = new Map<string, FileCheckState>();

  function visit(entry: ExportEntryDto, inherited: boolean): FileCheckState {
    const checked = !unpicked.has(entry.path) && (picked.has(entry.path) || inherited);
    let state: FileCheckState = checked ? "checked" : "unchecked";
    if (entry.dir) {
      const entries = children.get(entry.path);
      if (entries?.length) {
        let childState: FileCheckState | undefined;
        for (const child of entries) {
          const next = visit(child, checked);
          childState = childState === undefined || childState === next ? next : "mixed";
        }
        state = childState ?? state;
      } else if (!entries && (checked ? withUnpicked : withPicked).has(entry.path)) {
        state = "mixed";
      }
    }
    states.set(entry.path, state);
    return state;
  }

  for (const entry of children.get("") ?? []) visit(entry, false);
  return states;
}
