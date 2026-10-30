// 收藏的默认分组：按资源类型分
//
// 默认分组的成员是**现算**的：收藏项的类型决定它出现在哪一组，既不落盘、也不能拖入或移出 ——
// 把它拖进自建分组只是多一个归属，它仍留在自己那一类里。
// 身份用**固定 key**，不是分组名：分组名要跟界面语言走（中文「模组」/ 英文 "Mods"），
// 名字只在显示时按语言取。
//
// key 取全 0 开头、尾号递增的 uuid 形状：与用户自建的分组名撞不上。
// **一旦发布就不能改** —— 旧版本按这些 key 落过盘，清理要靠它认出来。
import { t } from "./i18n";
import zhCN from "./i18n/locales/zh-CN";
import enUS from "./i18n/locales/en-US";

/** 有默认分组的资源类型 → 固定 key，类型 ID 取 `FileType::to_string()` */
const DEFAULT_GROUP_KEYS: Record<string, string> = {
  modpack: "00000000-0000-0000-0000-0000000000a1",
  mod: "00000000-0000-0000-0000-0000000000a2",
  resourcepack: "00000000-0000-0000-0000-0000000000a3",
  shaderpack: "00000000-0000-0000-0000-0000000000a4",
  save: "00000000-0000-0000-0000-0000000000a5",
  dataPacks: "00000000-0000-0000-0000-0000000000a6",
};

/** 左栏固定列出的四个默认分组，顺序即分类栏顺序 */
export const FIXED_GROUP_TYPES = ["modpack", "mod", "resourcepack", "shaderpack"];

/** 有默认分组的资源类型 */
export const DEFAULT_GROUP_TYPES = [...FIXED_GROUP_TYPES, "save", "dataPacks"];

/**
 * 资源类型对应的默认分组 key
 *
 * - `fileType`: 资源类型 ID，如 modpack / mod / resourcepack / shaderpack
 *
 * # 返回值
 *
 * 固定 key；没有默认分组的类型返回 null，按未归组处理
 */
export function defaultGroupKey(fileType: string): string | null {
  return DEFAULT_GROUP_KEYS[fileType] ?? null;
}

/**
 * 默认分组 key 对应的资源类型
 *
 * - `key`: 分组 key
 *
 * # 返回值
 *
 * 资源类型 ID；不是默认分组时返回 null
 */
export function typeOfDefaultGroupKey(key: string): string | null {
  const hit = Object.entries(DEFAULT_GROUP_KEYS).find(([, value]) => value === key);
  return hit ? hit[0] : null;
}

/**
 * 这个分组 key 是不是默认分组
 *
 * 默认分组由收藏动作自动维护，删掉也会被补组再建回来，所以界面不给“删除分组”；
 * 组里的条目也就是“这一类收藏”，清空它等于删掉这些收藏。
 *
 * - `key`: 分组 key
 *
 * # 返回值
 *
 * 是默认分组时为 true
 */
export function isDefaultGroupKey(key: string): boolean {
  return typeOfDefaultGroupKey(key) !== null;
}

/**
 * 分组的显示名
 *
 * 默认分组按当前语言取文案，自建分组的 key 就是它自己的名字。
 *
 * - `key`: 分组 key
 *
 * # 返回值
 *
 * 界面上该显示的名字
 */
export function groupLabel(key: string): string {
  const fileType = typeOfDefaultGroupKey(key);
  return fileType ? t("collect.typeGroup." + fileType) : key;
}

/**
 * 旧口径下默认分组用过的名字 → 现在的固定 key
 *
 * 早先默认分组的 key 就是当时的界面语言文案，落过盘的用户数据里留着「模组」或 "Mods"。
 * 这里把两套语言的文案都收进来，第一次打开收藏窗口时据此把这些旧分组删掉。
 *
 * # 返回值
 *
 * 旧分组名到固定 key 的映射
 */
export function legacyDefaultGroupNames(): Map<string, string> {
  const map = new Map<string, string>();
  const catalogs = [zhCN, enUS] as Array<Record<string, string>>;
  for (const [fileType, key] of Object.entries(DEFAULT_GROUP_KEYS)) {
    for (const catalog of catalogs) {
      const name = catalog["collect.typeGroup." + fileType];
      if (name) map.set(name, key);
    }
  }
  return map;
}
