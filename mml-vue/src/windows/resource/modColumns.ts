// 模组表格视图的列定义（**唯一来源**）
//
// 宽度可拖、表头可点排序；拖出来的宽度存进**实例**的 `gui_setting.json`，
// 即 `Gui.ResourceModColWidths`，见后端 `gui_setting::GameViewSettingObj`。
// 后端只保管"列 key → 像素宽"这一串键值对，不认识有哪几列 ——
// 所以**新增 / 改名 / 删除列只改这个文件**，不必动 Rust。

import { t } from "../../lib/i18n";
import type { ModItemDto } from "../../lib/bindings";
import { sortedMods } from "./types";
/** 一列的规格 */
export interface ModCol {
  /** 取值键，见 ModTable 的 `colValue` */
  key: string;
  /** 表头文案的 i18n 键 */
  label: string;
  /** 默认宽度，单位 px */
  width: number;
  /** 拖宽下限，缺省用 `MIN_COL_WIDTH` */
  min?: number;
  /** 表头可点，在「正序 → 倒序 → 不排序」之间切（启用**右边**的列才给） */
  sortable?: boolean;
  /** 可拖右边缘改宽度（启用列固定，不给） */
  resizable?: boolean;
  /** 表头与内容居中（勾选框那列） */
  center?: boolean;
}

export const MOD_COLS: ModCol[] = [
  { key: "group", label: "resource.groupName", width: 150, resizable: true },
  // 启用列**不给拖宽**（勾选框列固定）：没有把手，也就没有那条竖线
  { key: "enable", label: "resource.colEnable", width: 44, center: true },
  { key: "name", label: "resource.modName", width: 200, sortable: true, resizable: true },
  { key: "note", label: "resource.modNote", width: 160, sortable: true, resizable: true },
  { key: "modId", label: "resource.colModId", width: 150, sortable: true, resizable: true },
  { key: "version", label: "resource.modVersion", width: 100, sortable: true, resizable: true },
  { key: "loader", label: "resource.colLoader", width: 110, sortable: true, resizable: true },
  { key: "side", label: "resource.colSide", width: 78, sortable: true, resizable: true },
  { key: "source", label: "resource.colSource", width: 90, sortable: true, resizable: true },
  { key: "projectId", label: "resource.colProjectId", width: 104, sortable: true, resizable: true },
  { key: "fileId", label: "resource.colFileId", width: 100, sortable: true, resizable: true },
  { key: "path", label: "resource.colPath", width: 260, sortable: true, resizable: true },
  { key: "author", label: "resource.modAuthor", width: 110, sortable: true, resizable: true },
  { key: "url", label: "resource.colUrl", width: 220, sortable: true, resizable: true },
];

/** 列宽下限，单位 px；再窄就只剩省略号了 */
export const MIN_COL_WIDTH = 60;

/** 上限：手改坏的 `gui_setting.json` 不该让表格没法看 */
const MAX_COL_WIDTH = 2000;

/**
 * 默认列宽表
 *
 * # 返回值
 *
 * 列 key → 默认像素宽
 */
export function defaultColWidths(): Record<string, number> {
  const out: Record<string, number> = {};
  for (const col of MOD_COLS) out[col.key] = col.width;
  return out;
}

/**
 * 把存下来的列宽收敛成一份可用的
 *
 * 只认**已知列**，改过名 / 删掉的列自然丢弃；宽度夹在 `[下限, 上限]` 内，
 * 缺项用默认值补齐 —— 于是"存过一半""存的是旧版本"都能安全落地。
 *
 * - `saved`: 从 `gui_setting.json` 读出来的原始值，可能是任何形状
 *
 * # 返回值
 *
 * 每个已知列都有的完整列宽表
 */
export function normalizeColWidths(saved: unknown): Record<string, number> {
  const out = defaultColWidths();
  if (!saved || typeof saved !== "object") return out;

  const raw = saved as Record<string, unknown>;
  for (const col of MOD_COLS) {
    const value = raw[col.key];
    if (typeof value !== "number" || !Number.isFinite(value)) continue;
    const min = col.min ?? MIN_COL_WIDTH;
    out[col.key] = Math.min(MAX_COL_WIDTH, Math.max(min, Math.round(value)));
  }
  return out;
}

/**
 * 表头排序状态
 *
 * `sort` 传 `null` 表示不排序，回到与列表视图一致的 modid 正序。
 */
export interface ModSort {
  /** 排序依据的列 key，与 `MOD_COLS` 的 key 对应 */
  key: string;
  /** 排序方向 */
  dir: "asc" | "desc";
}

// ---------- 排序口径（表格视图与"Shift 整段选择"共用） ----------
//
// 原来这套只写在 ModTable 里，于是 ModPane 算整段选择时只能用默认顺序 ——
// 表格按列排序后，选中范围会和看到的对不上（用户报过"整段选中 78 项"）。

/**
 * 加载器列的显示名
 *
 * 可能不止一个：多加载器包后端已汇总去重，这里逐个翻译后连起来。
 *
 * - `v`: 后端的加载器枚举名列表
 *
 * # 返回值
 *
 * 用 ` / ` 连起来的显示名，空列表得到空串
 */
export function loaderNames(v: string[]): string {
  return v.map((name) => t(`resource.loader.${name}`)).join(" / ");
}

/**
 * 加载侧的显示名
 *
 * 后端出稳定枚举名，文案在前端翻。
 *
 * - `v`: 后端的加载侧枚举名，空串表示未识别
 *
 * # 返回值
 *
 * 翻译后的显示名，`v` 为空时得到空串
 */
export function sideName(v: string): string {
  return v ? t(`resource.side.${v}`) : "";
}

/**
 * 下载源的显示名
 *
 * 与 `sideName` 同一口径：后端出稳定枚举名，文案在前端翻。
 *
 * - `v`: 后端的下载源枚举名，空串表示未识别
 *
 * # 返回值
 *
 * 翻译后的显示名，`v` 为空时得到空串
 */
export function sourceName(v: string): string {
  return v ? t(`resource.source.${v}`) : "";
}

/**
 * 取某一列的排序值
 *
 * - `item`: 模组条目
 * - `key`: 列 key，与 `MOD_COLS` 的 key 对应
 *
 * # 返回值
 *
 * 该列的字符串值；分组列、启用列与未知 key 都得到空串
 */
export function colValue(item: ModItemDto, key: string): string {
  switch (key) {
    case "name":
      return item.name || item.file;
    case "note":
      return item.note ?? "";
    case "modId":
      return item.modId;
    case "version":
      return item.version;
    case "loader":
      return loaderNames(item.loaders);
    case "side":
      return sideName(item.side);
    case "source":
      return sourceName(item.source);
    case "projectId":
      return item.projectId;
    case "fileId":
      return item.fileId;
    case "path":
      return item.path;
    case "author":
      return item.author;
    case "url":
      return item.url;
    default:
      return "";
  }
}

/**
 * 某一层的**显示顺序**：默认 modid 正序，点了表头就按那一列
 *
 * 数字列按数值比，其余按本地化字典序。
 *
 * - `items`: 这一层的模组
 * - `sort`: 表头排序状态，`null` 表示还没点过表头
 *
 * # 返回值
 *
 * 排好序的新数组，不改 `items`
 */
export function orderedMods(items: ModItemDto[], sort: ModSort | null): ModItemDto[] {
  const list = sortedMods(items);
  if (!sort) return list;

  const dir = sort.dir === "desc" ? -1 : 1;
  return [...list].sort((a, b) => {
    const va = colValue(a, sort.key);
    const vb = colValue(b, sort.key);
    const na = Number(va);
    const nb = Number(vb);
    const numeric = va !== "" && vb !== "" && !Number.isNaN(na) && !Number.isNaN(nb);
    const base = numeric
      ? na - nb
      : va.localeCompare(vb, undefined, { numeric: true, sensitivity: "base" });
    return base * dir;
  });
}