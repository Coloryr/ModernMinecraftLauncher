// 下载源的显示口径：页签顺序、标签、默认选哪个
//
// 下载整合包与添加资源两个窗口共用。顺序固定 CurseForge 在左、Modrinth 在右 ——
// 后端返回的是"支持哪些源"，顺序不代表界面顺序，所以这里重排一次。
// 没存过偏好时默认 CurseForge；偏好本身落 gui_config.json，见 lib/api.ts 的
// getAddModpackSource / getAddResourceSource（§7.1：用户数据不进 localStorage）。
import { t } from "./i18n";

/** 下载源线串 → i18n 文案键 */
const LABEL_KEYS: Record<string, string> = {
  curseforge: "modpack.curseforge",
  modrinth: "modpack.modrinth",
};

/** 页签顺序；不认识的源排在后面，保持后端给的相对顺序 */
const ORDER = ["curseforge", "modrinth"];

/** 没存过偏好时选中的源 */
const DEFAULT_SOURCE = "curseforge";

/**
 * 排好序、带显示名的下载源，直接给 SegmentedTabs 用
 *
 * - `list`: 后端返回的下载源线串
 *
 * # 返回值
 *
 * `value` 是线串，`label` 是翻译好的名字
 */
export function sourceOptions(list: string[]): Array<{ value: string; label: string }> {
  /** 排序名次：不在表里的整队排到最后 */
  const rank = (source: string) => {
    const index = ORDER.indexOf(source);
    return index < 0 ? ORDER.length : index;
  };
  return [...list]
    .sort((a, b) => rank(a) - rank(b))
    .map((value) => ({ value, label: t(LABEL_KEYS[value] ?? value) }));
}

/**
 * 这次该选哪个下载源
 *
 * - `list`: 已排序的下载源线串
 * - `saved`: 上次存的源，空串表示没存过
 *
 * # 返回值
 *
 * 存过的还在就用它，否则默认 CurseForge，再不支持就退第一个；列表为空时是空串
 */
export function pickSource(list: string[], saved: string): string {
  if (saved && list.includes(saved)) return saved;
  if (list.includes(DEFAULT_SOURCE)) return DEFAULT_SOURCE;
  return list[0] ?? "";
}
