// 收藏的默认分组：按资源类型分
//
// 新收藏在星标那一刻就归组，分组名按当时的界面语言落盘 —— 分组名是用户数据，
// 之后切换语言不会改名。资源类型 ID 取 `FileType::to_string()`。
import { t } from "./i18n";

/**
 * 资源类型对应的默认分组名
 *
 * - `fileType`: 资源类型 ID，如 modpack / mod / resourcepack / shaderpack
 *
 * # 返回值
 *
 * 分组名；没有对应默认分组的类型返回 null（按未归组处理）
 */
export function typeGroupName(fileType: string): string | null {
  const key = `collect.typeGroup.${fileType}`;
  const name = t(key);
  return name === key ? null : name;
}

/** 左栏固定列出的四个默认分组，顺序即分类栏顺序 */
export const FIXED_GROUP_TYPES = ["modpack", "mod", "resourcepack", "shaderpack"];

/** 有默认分组的资源类型 */
const DEFAULT_GROUP_TYPES = [...FIXED_GROUP_TYPES, "save", "dataPacks"];

/**
 * 这个分组名是不是按类型自动分的默认分组
 *
 * 默认分组由收藏动作自动维护，删掉也会被补组再建回来，所以界面不给"删除分组"；
 * 组里的条目也就是"这一类收藏"，清空它等于删掉这些收藏。
 *
 * - `name`: 分组名
 *
 * # 返回值
 *
 * 是默认分组时为 true
 */
export function isDefaultGroupName(name: string): boolean {
  return DEFAULT_GROUP_TYPES.some((fileType) => typeGroupName(fileType) === name);
}
