// ESLint 扁平配置：ESLint 10 / eslint-plugin-vue 10 / typescript-eslint 8
//
// 定位是**拦真问题、不挑格式**：只上 vue 的 essential 规则集 + js/ts 的 recommended，
// 不引入 recommended 里那一大批排版规则，比如属性换行、缩进、标签顺序——那类规则在本仓库
// 会刷出上千条噪音，而格式一致性靠 vue-tsc 与人工 review 已经够用。
//
// 跑法：cd mml-vue && npm run lint（加 --fix 自动修可修的项）
import js from "@eslint/js";
import pluginVue from "eslint-plugin-vue";
import tseslint from "typescript-eslint";
import globals from "globals";

export default tseslint.config(
  {
    ignores: [
      "dist/**",
      "node_modules/**",
    ],
  },
  js.configs.recommended,
  tseslint.configs.recommended,
  pluginVue.configs["flat/essential"],
  {
    // .vue 的 <script lang="ts"> 要交给 TS 解析器
    files: ["**/*.vue"],
    languageOptions: {
      parserOptions: { parser: tseslint.parser },
    },
  },
  {
    languageOptions: {
      globals: { ...globals.browser },
    },
    rules: {
      // 临时调试用的 console.log 不该留在仓库里；warn / error 是正式诊断，放行
      "no-console": ["warn", { allow: ["warn", "error"] }],
      "no-debugger": "error",
      // 未使用变量主要由 tsc 的 noUnusedLocals 在构建时拦下；这里兜住参数与 catch 子句
      "@typescript-eslint/no-unused-vars": [
        "warn",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_", caughtErrors: "none" },
      ],
      // 设置窗口的 tab 是**有意**共享同一份设置对象（父级建、子级就地改），
      // 所以这里降为警告而不是错误：它标出的是"哪天要重构成 emit 式更新"的位置，
      // 而不是当前就一定写错了。改成 error 会让 npm run lint 一直红着、失去信号意义。
      "vue/no-mutating-props": "warn",
    },
  },
);
