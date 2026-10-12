// 列表卡片上的项目标签：一行宽度有限，标签多了必须让位给省略号
//
// 详情页不受这个上限约束（那边允许换行，标签全都能看到），只有卡片这一行要用。
import type { TagDto } from "./bindings";

/**
 * 卡片上最多显示的标签数
 *
 * 主窗口 920px 宽时，卡片标签行的可用宽度约 730px，标签平均 80–96px 一个，
 * 6 个＋省略号还有余量；再往上就会在窄窗口里重新被裁成半截。
 */
export const CARD_TAG_LIMIT = 6;

/**
 * 卡片上要渲染的标签
 *
 * - `tags`: 项目标签，顺序由后端排好，本函数不动顺序
 *
 * # 返回值
 *
 * - `shown`: 实际渲染的标签，最多 [`CARD_TAG_LIMIT`] 个
 * - `more`: 是否还有被略掉的标签，为真时末尾补一个省略号
 */
export function cardTags(tags: TagDto[]): { shown: TagDto[]; more: boolean } {
  return {
    shown: tags.slice(0, CARD_TAG_LIMIT),
    more: tags.length > CARD_TAG_LIMIT,
  };
}
