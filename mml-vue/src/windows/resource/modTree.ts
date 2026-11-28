import type { ModItemDto, ModMixinTargetDto } from "../../lib/bindings";
import { t } from "../../lib/i18n";
import { modRowKey, patchHit, patchKindLabel } from "./types";

/** 两种视图共用行键和层级，只有模组行参与选择或编辑。 */
export type ModTreeRow = { key: string; depth: number } & (
  | { kind: "mod"; item: ModItemDto; expandable: boolean; open: boolean }
  | { kind: "patches"; parent: string; count: number; open: boolean }
  | { kind: "patch"; asm: boolean; name: string; detail: string; title: string }
);

interface ModTreeOptions {
  query: string;
  order: (items: ModItemDto[]) => ModItemDto[];
  isOpen: (key: string, item: ModItemDto) => boolean;
  isPatchesOpen: (key: string) => boolean;
}

/** 同一目标类的 Mixin 合并为一行，悬停时显示全部来源。 */
function patchRowsOf(item: ModItemDto, parent: string, depth: number, query: string) {
  const groups = new Map<string, ModMixinTargetDto[]>();
  for (const target of item.mixins) {
    const bucket = groups.get(target.target);
    if (bucket) bucket.push(target);
    else groups.set(target.target, [target]);
  }

  const rows: Extract<ModTreeRow, { kind: "patch" }>[] = [];
  for (const [name, targets] of groups) {
    const hit = name + " " + targets.map((target) => target.method + " " + target.mixin).join(" ");
    if (query && !hit.toLowerCase().includes(query)) continue;
    const detail = targets.map((target) => target.method + " · " + patchKindLabel(target.kind)).join("，");
    const sources = [...new Set(targets.map((target) => target.mixin).filter(Boolean))].join("、");
    const title = [name, detail, sources && t("resource.modPatchSources", { sources })].filter(Boolean).join("\n");
    rows.push({ kind: "patch", key: `${parent}|mixin|${name}`, depth, asm: false, name, detail, title });
  }

  for (const [index, target] of item.asm.entries()) {
    const hit = [target.source, target.transformer, target.target, target.method].join(" ");
    if (query && !hit.toLowerCase().includes(query)) continue;
    const name = target.target || target.transformer;
    const detail = [target.method, target.source].filter(Boolean).join(" · ");
    rows.push({
      kind: "patch", key: `${parent}|asm|${index}`, depth, asm: true, name, detail,
      title: target.transformer + " · " + detail,
    });
  }
  return rows;
}

/**
 * 按展开状态生成虚拟列表的行，改写目标在内置模组之后另占一层。
 * - `items`: 当前分组的顶层模组
 * - `depth`: 顶层模组的缩进层级
 * - `options`: 视图的搜索词、排序和展开状态
 * # 返回值
 * 按显示顺序排列的模组、改写目标分组及目标详情。
 */
export function modTreeRows(items: ModItemDto[], depth: number, options: ModTreeOptions): ModTreeRow[] {
  const rows: ModTreeRow[] = [];
  function append(children: ModItemDto[], level: number, parent: string) {
    for (const item of options.order(children)) {
      const key = parent ? `${parent}|${modRowKey(item)}` : modRowKey(item);
      const expandable = !!(item.jarInJar.length || item.mixins.length || item.asm.length);
      const open = expandable && options.isOpen(key, item);
      rows.push({ kind: "mod", key, depth: level, item, expandable, open });
      if (!open) continue;

      if (item.jarInJar.length) append(item.jarInJar, level + 1, key);
      if (!item.mixins.length && !item.asm.length) continue;
      // 搜模组名后仍可查看全部目标；直接搜类或方法时只列命中的目标。
      const patchQuery = options.query && patchHit(item, options.query) ? options.query : "";
      const patches = patchRowsOf(item, key, level + 2, patchQuery);
      if (!patches.length) continue;
      const openPatches = options.isPatchesOpen(key);
      rows.push({
        kind: "patches", key: key + "|patches", depth: level + 1,
        parent: key, count: patches.length, open: openPatches,
      });
      if (openPatches) {
        for (const patch of patches) rows.push(patch);
      }
    }
  }
  append(items, depth, "");
  return rows;
}
