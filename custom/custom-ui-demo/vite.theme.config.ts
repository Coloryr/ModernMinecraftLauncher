import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";

// 整页包的窗口组件入口：IIFE，Vue 从页面全局 __mmlVue 取；
// 产物 custom.js 与 style.css 会被 pack.mjs 放进 zip 根目录（custom.css）。
// Tailwind 只出 utilities：token 来自 mml-vue/tailwind.css，取的是启动器主题变量。
export default defineConfig({
  plugins: [vue(), tailwindcss()],
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
