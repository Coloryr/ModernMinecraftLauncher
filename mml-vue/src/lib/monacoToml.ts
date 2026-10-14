// TOML 语法：Monaco 自带的语言定义里没有 toml（0.56 的 languages/definitions 下 81 种，
// 最接近的只有 ini），所以这里按 Monarch 自己注册一份。
//
// 只管**上色**，不做语义校验（与 Monaco 那些自带语言一致）；用的是 Monarch 的着色器，
// 与 json 那套需要 worker 的语言服务无关。
//
// 覆盖 TOML v1.0 的主要形态：注释、表头 [a.b] 与数组表头 [[a.b]]、裸键与点分键、
// 基本/字面/多行字符串、整数（含 0x / 0o / 0b 与下划线分隔）、浮点（含 inf / nan）、
// 布尔、RFC 3339 日期时间、数组与内联表。
import { monaco } from "./monaco";

/** 语言 id，ConfigWindow 的文件类型映射里用的就是它 */
const ID = "toml";

/** 注册 TOML：语言本体 + 注释 / 括号配置 + 着色器 */
export function registerToml() {
  monaco.languages.register({ id: ID, extensions: [".toml"], aliases: ["TOML", "toml"] });

  monaco.languages.setLanguageConfiguration(ID, {
    comments: { lineComment: "#" },
    brackets: [
      ["[", "]"],
      ["{", "}"],
      ["(", ")"],
    ],
    autoClosingPairs: [
      { open: "[", close: "]" },
      { open: "{", close: "}" },
      { open: "(", close: ")" },
      { open: '"', close: '"' },
      { open: "'", close: "'" },
    ],
    surroundingPairs: [
      { open: "[", close: "]" },
      { open: "{", close: "}" },
      { open: "(", close: ")" },
      { open: '"', close: '"' },
      { open: "'", close: "'" },
    ],
  });

  monaco.languages.setMonarchTokensProvider(ID, {
    defaultToken: "",
    tokenPostfix: ".toml",
    tokenizer: {
      root: [
        [/#.*$/, "comment"],
        // 表头：数组表头写在单层表头前面
        [/\[\[/, { token: "delimiter.bracket", next: "@arrayTable" }],
        [/\[/, { token: "delimiter.bracket", next: "@table" }],

        // 键：点分裸键（a.b 与 "a b" 两种写法）靠 = 收尾，避免把右侧的值也吃掉
        [/(?:[A-Za-z0-9_-]+)(?:\s*\.\s*[A-Za-z0-9_-]+)*(?=\s*=)/, "type"],
        [/"(?:[^"\\]|\\.)*"(?=\s*=)/, "type"],
        [/'(?:[^']*)'(?=\s*=)/, "type"],

        // 字符串：三引号在前，否则 "" 会被当成空串而漏掉后面
        [/"""/, { token: "string.quote", next: "@mlBasic" }],
        [/'''/, { token: "string.quote", next: "@mlLiteral" }],
        [/"/, { token: "string.quote", next: "@basic" }],
        [/'/, { token: "string.quote", next: "@literal" }],

        // 日期时间：RFC 3339 的日期，带不带时间与偏移都认
        [/\d{4}-\d{2}-\d{2}(?:[Tt ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:[Zz]|[+-]\d{2}:\d{2})?)?/, "number.date"],
        [/\d{2}:\d{2}:\d{2}(?:\.\d+)?/, "number.date"],

        // 数字：进制前缀与 inf / nan 都在常规浮点之前
        [/0x[0-9A-Fa-f_]+/, "number.hex"],
        [/0o[0-7_]+/, "number.octal"],
        [/0b[01_]+/, "number.binary"],
        [/[+-]?(?:inf|nan)/, "number"],
        [/[+-]?\d[\d_]*(?:\.\d[\d_]*)?(?:[eE][+-]?\d+)?/, "number"],

        [/\b(?:true|false)\b/, "keyword"],
        [/=/, "operator"],
        [/[\[\]{}]/, "delimiter.bracket"],
        [/,/, "delimiter"],
      ],

      table: [
        [/[^\]\n]+/, "type"],
        [/\]/, { token: "delimiter.bracket", next: "@pop" }],
      ],

      arrayTable: [
        [/[^\]\n]+/, "type"],
        [/\]\]/, { token: "delimiter.bracket", next: "@pop" }],
      ],

      // 字符串各状态：多行串要先认三引号，否则会把开头两个引号当成空串
      basic: [
        [/[^"\\]+/, "string"],
        [/\\./, "string.escape"],
        [/"/, { token: "string.quote", next: "@pop" }],
      ],

      literal: [
        [/[^']+/, "string"],
        [/'/, { token: "string.quote", next: "@pop" }],
      ],

      mlBasic: [
        [/[^"\\]+/, "string"],
        [/\\./, "string.escape"],
        [/"""/, { token: "string.quote", next: "@pop" }],
        [/"/, "string"],
      ],

      mlLiteral: [
        [/[^']+/, "string"],
        [/'''/, { token: "string.quote", next: "@pop" }],
        [/'/, "string"],
      ],
    },
  });
}
