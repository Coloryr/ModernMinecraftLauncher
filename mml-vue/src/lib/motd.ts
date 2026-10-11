// MOTD 的展示辅助，纯函数，主窗口的 MOTD 卡片与资源窗口的服务器列表共用
//
// 后端把 MOTD 解析成"带样式的文字段"，即 `MotdSegmentDto`：文字 + 颜色 +
// 加粗 / 斜体 / 下划线 / 删除线。这里只负责把它翻成内联样式，
// 两处各写一遍迟早会漂移成"一个能上色一个不能"。
import type { MotdDto, MotdSegmentDto } from "./bindings";

/**
 * 一段 MOTD 文字的内联样式，含颜色与加粗 / 斜体 / 下划线 / 删除线
 *
 * **颜色为空串时不写 `color`** —— 那表示"MOTD 没指定颜色"，让它继承所在处的文字色。
 * 后端就是这么约定的，起始色是空串而不是白色；写死白色的话，浅色主题下这段文字
 * 会变成白底白字看不见。
 *
 * - `seg`: 后端解析出的文字段
 *
 * # 返回值
 *
 * Vue 可直接绑定的样式对象
 */
export function motdSegStyle(seg: MotdSegmentDto): Record<string, string> {
  const deco = [seg.underlined ? "underline" : "", seg.strikethrough ? "line-through" : ""]
    .filter(Boolean)
    .join(" ");
  const style: Record<string, string> = {
    fontWeight: seg.bold ? "700" : "inherit",
    fontStyle: seg.italic ? "italic" : "inherit",
    textDecoration: deco || "none",
  };
  if (seg.color) style.color = seg.color;
  return style;
}

/**
 * 服务器图标：Base64 PNG → data URI，直接喂 `<img>`
 *
 * 后端给的可能是裸 base64，也可能已经是 data URI，两种都认。
 *
 * - `m`: MOTD 查询结果，可能为 null
 *
 * # 返回值
 *
 * data URI；没有图标时为 `null`
 */
export function faviconOf(m: MotdDto | null | undefined): string | null {
  const f = m?.favicon;
  if (!f) return null;
  return f.startsWith("data:") ? f : `data:image/png;base64,${f}`;
}
