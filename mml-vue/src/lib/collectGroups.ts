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