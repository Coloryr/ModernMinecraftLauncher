// Minecraft 文本格式（`§` 颜色码）的**显示期**解析
//
// 用途：材质包 / 数据包的描述、存档名这类字段里可能带 `§a` `§l` 这种格式码
// （整合包作者常用来上色）。它们**只影响显示**：
// 原始字符串原样保留在 DTO 里，不在这里改动、也不回写 —— 所以这个模块只产出
// "渲染用的片段"，调用方拿去铺 `<span>`。
//
// 算法与内核 `mml_game::game_motd::chat_from_plain` 一致，同一套颜色表、
// 同样的"颜色码会重置字体样式"规则，保证 MOTD 卡片与这里的观感统一。
// 内核那边是解析 MOTD 的 Chat JSON（要发给前端的是结构化的段），这里只需要
// 把一段纯文本切好，所以不共用代码、只共用规则。

/** 一段带样式的文字 */
export interface FormattedSegment {
  /** 文字内容，不含格式码 */
  text: string;
  /** 颜色 #RRGGBB；空串表示跟随当前文字颜色 */
  color: string;
  /** 是否加粗 */
  bold: boolean;
  /** 是否斜体 */
  italic: boolean;
  /** 是否下划线 */
  underlined: boolean;
  /** 是否删除线 */
  strikethrough: boolean;
}

/** `§` 颜色码 → #RRGGBB，与内核 `code_color` 同一张表 */
const CODE_COLORS: Record<string, string> = {
  "0": "#000000",
  "1": "#0000AA",
  "2": "#00AA00",
  "3": "#00AAAA",
  "4": "#AA0000",
  "5": "#AA00AA",
  "6": "#FFAA00",
  "7": "#AAAAAA",
  "8": "#555555",
  "9": "#5555FF",
  a: "#55FF55",
  b: "#55FFFF",
  c: "#FF5555",
  d: "#FF55FF",
  e: "#FFFF55",
  f: "#FFFFFF",
};

/**
 * 把带 `§` 格式码的文本切成渲染片段
 *
 * 认得出的码按下面这套规则处理：
 *
 * - `§r` 重置全部样式；
 * - 颜色码（`§0`–`§f`）**同时重置字体样式** —— 这是原版行为，不照做会出现
 *   "换了颜色但还带着上一段的加粗"；
 * - `§k`（混淆）当普通文字处理：真做混淆要逐帧随机，静态列表里没意义；
 * - 认不出的码（`§x` 之类）按字面 `§x` 显示，别把作者写错的东西吞掉；
 * - 没有任何格式码时返回**单个无样式片段**，调用方不必特判。
 *
 * - `text`: 原始文本，`§` 格式码按字面出现在里面
 *
 * # 返回值
 *
 * 按样式切好的片段，按原文顺序排列
 */
export function parseFormatting(text: string): FormattedSegment[] {
  const out: FormattedSegment[] = [];
  let color = "";
  let bold = false;
  let italic = false;
  let underlined = false;
  let strikethrough = false;
  let current = "";

  /** 把攒下的文字收成一段；空的不收，免得前端多出空 span */
  function flush() {
    if (!current) return;
    out.push({ text: current, color, bold, italic, underlined, strikethrough });
    current = "";
  }

  /** 重置字体样式（颜色码与 `§r` 都要做） */
  function resetStyles() {
    bold = false;
    italic = false;
    underlined = false;
    strikethrough = false;
  }

  /** 字体样式码；只影响之后的文字 */
  const styleCodes: Record<string, () => void> = {
    l: () => (bold = true),
    o: () => (italic = true),
    n: () => (underlined = true),
    m: () => (strikethrough = true),
    // 混淆字符按普通文字展示（见函数说明）
    k: () => {},
  };

  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (ch !== "§") {
      current += ch;
      continue;
    }

    // `§` 后面没字符了：按字面留着
    const code = text[i + 1];
    if (code === undefined) {
      current += ch;
      break;
    }
    i++; // 消费掉码本身

    const lower = code.toLowerCase();
    const hex = CODE_COLORS[lower];
    if (hex) {
      // 换颜色：先收尾上一段，再重置字体样式（原版行为）
      flush();
      color = hex;
      resetStyles();
      continue;
    }
    if (lower === "r") {
      flush();
      color = "";
      resetStyles();
      continue;
    }

    // 字体样式码：同样要先收尾，样式只影响之后的文字
    const apply = styleCodes[lower];
    if (apply) {
      flush();
      apply();
      continue;
    }

    // 认不出的码：原样显示
    current += ch + code;
  }

  flush();
  return out;
}

/**
 * 片段 → 内联样式，含颜色与加粗 / 斜体 / 下划线 / 删除线
 *
 * - `seg`: 待转换的片段
 *
 * # 返回值
 *
 * Vue 可直接绑定的样式对象
 */
export function segmentStyle(seg: FormattedSegment): Record<string, string> {
  const deco = [seg.underlined ? "underline" : "", seg.strikethrough ? "line-through" : ""]
    .filter(Boolean)
    .join(" ");
  const style: Record<string, string> = {
    fontWeight: seg.bold ? "700" : "inherit",
    fontStyle: seg.italic ? "italic" : "inherit",
    textDecoration: deco || "none",
  };
  // 没指定颜色时不写 color：让它继承所在处，写死白色会在浅色主题下跳出来
  if (seg.color) style.color = seg.color;
  return style;
}

/**
 * 去掉格式码、只留纯文本
 *
 * 给不适合上色的地方用：`title` 悬浮提示、确认弹窗里的名字、搜索匹配等，
 * 那些地方 `§a` 只会显示成一串乱码。认不出的码按 `parseFormatting` 的口径原样保留。
 *
 * - `text`: 带格式码的原始文本
 *
 * # 返回值
 *
 * 去掉认得出的格式码后的纯文本
 */
export function stripFormatting(text: string): string {
  return parseFormatting(text)
    .map((seg) => seg.text)
    .join("");
}
