import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";

// 入口脚本按 IIFE 打包，Vue 从页面全局取（mml-vue 暴露 window.__mmlVue），
// 包里只带组件代码，不再打一份 Vue。
// 样式源码 src/custom.css 走同一个构建：Tailwind 把 @apply 展开、token 落成变量，
// 产物按下面的 cssFileName 叫 custom.css，由 scripts/pack.ts 复制到包根。
export default defineConfig({
  plugins: [vue(), tailwindcss()],
  build: {
    lib: {
      entry: "src/replace.ts",
      formats: ["iife"],
      name: "MmlCustomCssEntry",
      fileName: () => "custom.js",
      // 包根要的样式文件名（默认会叫 style.css）
      cssFileName: "custom",
    },
    outDir: "dist",
    emptyOutDir: true,
    minify: false,
    rollupOptions: {
      external: ["vue"],
      output: { globals: { vue: "__mmlVue" } },
    },
  },
});
