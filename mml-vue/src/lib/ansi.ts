// ANSI SGR（颜色 / 粗体 / 下划线）解析
//
// 启动前 / 后命令的输出带色码（`ESC[96m` 这类），日志窗口要按原色显示，所以正文先剥掉转义，
// 颜色改成 Monaco 的内联装饰类补在对应区间上。只认 16 色调色板与几种属性；256 色 / 真彩色
// 不认，那几段按默认色显示，转义照样剥掉。

/** 一段带样式的区间 */
export interface AnsiSegment {
  /** 起始列，0 起；按 UTF-16 索引，与 Monaco 的列一一对应 */
  start: number;
  /** 结束列，不含 */
  end: number;
  /** 样式类名，空格分隔 */
  classes: string;
}

/** 一行解析结果 */
export interface AnsiText {
  /** 剥掉转义后的正文 */
  text: string;
  /** 需要着色的区间，按出现顺序排列 */
  segments: AnsiSegment[];
}

/** ESC 字符；正则里直接写控制字符会被 no-control-regex 拦下，用码点拼 */
const ESC = String.fromCharCode(27);

/** SGR 转义：`ESC[0m` / `ESC[1;96m` 这类 */
const SGR = new RegExp(`${ESC}\\[([0-9;]*)m`, "g");

/** 当前生效的样式 */
interface AnsiState {
  /** 前景色类名；空串表示默认色 */
  fg: string;
  /** 加粗 */
  bold: boolean;
  /** 变暗 */
  dim: boolean;
  /** 下划线 */
  underline: boolean;
}

/** 把当前样式拼成类名；没有任何样式时是空串 */
function classesOf(state: AnsiState): string {
  const out: string[] = [];
  if (state.fg) out.push(state.fg);
  if (state.bold) out.push("ansi-bold");
  if (state.dim) out.push("ansi-dim");
  if (state.underline) out.push("ansi-underline");
  return out.join(" ");
}

/** 应用一条 SGR 的参数表 */
function applyParams(state: AnsiState, params: string[]) {
  for (let i = 0; i < params.length; i++) {
    const code = Number(params[i] || "0");
    if (code === 0) {
      state.fg = "";
      state.bold = false;
      state.dim = false;
      state.underline = false;
    } else if (code === 1) {
      state.bold = true;
    } else if (code === 2) {
      state.dim = true;
    } else if (code === 4) {
      state.underline = true;
    } else if (code === 22) {
      state.bold = false;
      state.dim = false;
    } else if (code === 24) {
      state.underline = false;
    } else if (code === 39) {
      state.fg = "";
    } else if ((code >= 30 && code <= 37) || (code >= 90 && code <= 97)) {
      state.fg = `ansi-fg-${code}`;
    } else if (code === 38 || code === 48) {
      // 扩展色：38;5;n 跟两个参数，38;2;r;g;b 跟四个；这里只按长度跳过
      const kind = params[i + 1];
      i += kind === "5" ? 2 : kind === "2" ? 4 : 0;
    }
  }
}

/**
 * 剥掉 ANSI 转义并算出要着色的区间
 *
 * - `raw`: 原始日志行，可能带色码
 *
 * # 返回值
 *
 * 正文与区间；没有色码时正文原样返回、区间为空
 */
export function parseAnsi(raw: string): AnsiText {
  if (!raw.includes(ESC)) return { text: raw, segments: [] };

  const state: AnsiState = { fg: "", bold: false, dim: false, underline: false };
  const segments: AnsiSegment[] = [];
  let text = "";
  let segStart = 0;
  let segClasses = "";

  /** 收掉上一段，按当前样式开下一段 */
  const flush = () => {
    if (text.length > segStart && segClasses) {
      segments.push({ start: segStart, end: text.length, classes: segClasses });
    }
    segStart = text.length;
    segClasses = classesOf(state);
  };

  let last = 0;
  for (const match of raw.matchAll(SGR)) {
    const index = match.index ?? 0;
    text += raw.slice(last, index);
    last = index + match[0].length;
    applyParams(state, match[1] ? match[1].split(";") : ["0"]);
    flush();
  }
  text += raw.slice(last);

  if (text.length > segStart && segClasses) {
    segments.push({ start: segStart, end: text.length, classes: segClasses });
  }

  return { text, segments };
}
