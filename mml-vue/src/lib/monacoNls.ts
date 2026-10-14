// Monaco 界面文案（查找框 / 补全提示等）的中文表
//
// Monaco 的 `localize(index, "English", …)` 会先查 `globalThis._VSCODE_NLS_MESSAGES[index]`，
// 查不到才回落到英文原文（见 monaco-editor/esm/vs/nls.js 的 lookupMessage）。npm 包**不带
// 任何语言包**，所以默认恒为英文；这里就用它自己的机制，在开窗前把这张表挂到那个全局上。
// lookup 是每次现查的，所以语言一变重新挂一次即时生效，不需要重启窗口。
//
// 下标取自 Monaco 0.56 的 esm 源码（findWidget / findController / suggestWidget /
// suggestController 里 `localize(<下标>, "英文")` 的第一个参数）。**升级 Monaco 后这些下标
// 可能变**，届时按同一方法重新抽一遍；抽漏了不会出错，只是那一条显示英文。
//
// 表里只放"编辑器里真会看见"的文案：查找框自身、查找/替换的提示与通知、补全框。
// Monaco 的命令名与配色项不在此列 —— 那些只出现在命令面板 / 主题编辑里，本仓库都没有。

/** 下标 → 中文；行尾注释是 Monaco 里的英文原文，便于升级后核对 */
const ZH: Record<number, string> = {
  // ---- 查找框（contrib/find/findWidget）----
  1003: "查找 / 替换", // Find / Replace
  1004: "查找", // Find
  1005: "查找", // Find
  1006: "上一个匹配", // Previous Match
  1007: "下一个匹配", // Next Match
  1008: "在选区中查找", // Find in Selection
  1009: "关闭", // Close
  1010: "替换", // Replace
  1011: "替换", // Replace
  1012: "替换", // Replace
  1013: "全部替换", // Replace All
  1014: "切换替换", // Toggle Replace
  1015: "只高亮前 {0} 个结果，全部替换等操作仍在全文上生效。", // Only the first {0} results are highlighted…
  1016: "{0} / {1}", // {0} of {1}
  1017: "没有结果", // No results
  1018: "找到 {0} 个", // {0} found
  1019: "为 '{1}' 找到 {0} 个", // {0} found for '{1}'
  1020: "为 '{1}' 找到 {0} 个，位于 {2}", // {0} found for '{1}', at {2}
  1021: "为 '{1}' 找到 {0} 个", // {0} found for '{1}'
  1022: "按 {0} 查看无障碍帮助", // Press {0} for accessibility help
  1023: "{0}，{1}", // {0}, {1}
  1024: "{0}，{1}", // {0}, {1}

  // ---- 查找 / 替换的提示与通知（contrib/find/findController）----
  979: "文件太大，无法执行全部替换。", // The file is too large to perform a replace all operation.
  980: "&&查找", // &&Find
  981: "为 '{1}' 找到 {0} 个", // {0} found for '{1}'
  982: "没有匹配项，换个词试试。", // No matches. Try searching for something else.
  983: "输入数字跳到指定匹配项，范围 1 到 {0}", // Type a number to go to a specific match…
  984: "请输入 1 到 {0} 之间的数字", // Please type a number between 1 and {0}
  985: "请输入 1 到 {0} 之间的数字", // Please type a number between 1 and {0}
  986: "&&替换", // &&Replace
  987: "查找", // Find
  988: "带参数的查找", // Find with Arguments
  989: "在选区中查找", // Find with Selection
  990: "查找下一个", // Find Next
  991: "查找上一个", // Find Previous
  992: "跳到匹配项…", // Go to Match...
  993: "查找下一个选区", // Find Next Selection
  994: "查找上一个选区", // Find Previous Selection
  995: "替换", // Replace

  // ---- 补全框（contrib/suggest）----
  1514: "接受 '{0}' 时另做了 {1} 处修改", // Accepting '{0}' made {1} additional edits
  1515: "插入", // Insert
  1516: "插入", // Insert
  1517: "替换", // Replace
  1518: "替换", // Replace
  1519: "插入", // Insert
  1520: "选择", // Select
  1521: "收起", // Show Less
  1522: "展开", // Show More
  1523: "触发补全", // Trigger Suggest
  1524: "重置补全框大小", // Reset Suggest Widget Size
  1534: "正在加载…", // Loading...
  1535: "没有建议。", // No suggestions.
  1536: "建议", // Suggest
  1541: "{0}，文档：{1}", // {0}, docs: {1}
};

declare global {
  /** Monaco 的文案表：nls.js 的 lookupMessage 按数字下标来这里查 */
  var _VSCODE_NLS_MESSAGES: Record<number, string> | undefined;
  /** 语言标记，Monaco 只用它判断 pseudo 调试模式 */
  var _VSCODE_NLS_LANGUAGE: string | undefined;
}

/**
 * 按当前语言挂上 Monaco 的文案表
 *
 * - `lang`: 界面语言；只有中文界面才挂表，英文界面挂空表走 Monaco 自己的英文原文
 *
 * 挂空表而不是删掉：lookup 每次都读这个全局，换一次即时生效，不用重开窗口
 */
export function applyMonacoNls(lang: "zh_cn" | "en_us") {
  globalThis._VSCODE_NLS_MESSAGES = lang === "zh_cn" ? { ...ZH } : {};
  globalThis._VSCODE_NLS_LANGUAGE = lang === "zh_cn" ? "zh-cn" : "en";
}
