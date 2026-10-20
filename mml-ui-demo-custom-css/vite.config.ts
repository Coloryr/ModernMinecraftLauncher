import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// 入口脚本按 IIFE 打包，Vue 从页面全局取（mml-vue 暴露 window.__mmlVue），
// 包里只带组件代码，不再打一份 Vue
export default defineConfig({
  plugins: [vue()],
  build: {
    lib: {
      entry: "src/replace.ts",
      formats: ["iife"],
      name: "MmlCustomCssEntry",
      fileName: () => "custom.js",
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
