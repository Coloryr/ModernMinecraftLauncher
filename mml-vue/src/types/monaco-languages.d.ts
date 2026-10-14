// Monaco 的语言包与编辑器贡献以纯 JS 交付（整包只给一份 monaco.d.ts），深路径导入要让 TS 放行。
// 路径按包的 exports 映射写：`monaco-editor/*` → `esm/vs/*.js`，所以**不要**带 `esm/vs/` 前缀。
// 通配符每种只能有一个 `*`，所以按目录前缀声明；**不要**写成 "monaco-editor/*" ——
// 那会把别处（如 editor.api）的真类型一起挡掉。
declare module "monaco-editor/languages/definitions/*/register.js";
declare module "monaco-editor/language/json/monaco.contribution.js" {
  /** JSON 语言服务的诊断选项；注释与尾逗号放行，见 lib/monacoCode 里的用法 */
  export const jsonDefaults: {
    setDiagnosticsOptions(options: {
      /** 是否做语法校验 */
      validate?: boolean;
      /** 允许行注释与块注释，json5 需要 */
      allowComments?: boolean;
      /** 尾逗号的处置，json5 取 `ignore` */
      trailingCommas?: "ignore" | "error" | "warning";
      /** JSON Schema 列表，本仓库不用 */
      schemas?: unknown[];
    }): void;
  };
}

/** 按需注册的编辑器贡献：折叠 / 括号配对 / 补全 / 悬停 / 选中词高亮 / 自动缩进 */
declare module "monaco-editor/editor/contrib/*";

/** 按需注册的编辑器特性：查找（Ctrl+F） */
declare module "monaco-editor/features/*";
