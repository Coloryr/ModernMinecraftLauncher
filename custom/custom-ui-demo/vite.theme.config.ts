import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// 整页包的窗口组件入口：IIFE，Vue 从页面全局 __mmlVue 取；
// 产物 custom.js 与 style.css 会被 pack.mjs 放进 zip 根目录（custom.css）。
export default defineConfig({
  plugins: [vue()],
  build: {
    lib: {
      entry: "src/replace.ts",
      formats: ["iife"],
      name: "MmlWindowTheme",
      fileName: () => "custom.js",
    },
    outDir: "dist-theme",
    emptyOutDir: true,
    minify: false,
    cssCodeSplit: false,
    rollupOptions: {
      external: ["vue"],
      output: { globals: { vue: "__mmlVue" } },
    },
  },
});
