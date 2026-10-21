import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import prefixer from "postcss-prefix-selector";

// 自定义主页的组件入口：IIFE，Vue 从页面全局 __mmlVue 取；
// 产物 custom.js / custom.css 由 pack.mjs 放进 zip 根目录。
export default defineConfig({
  plugins: [vue()],
  // 组件直接跑在宿主里，样式必须限定在主页容器内，不能改到启动器其它窗口
  css: {
    postcss: {
      plugins: [
        prefixer({
          prefix: ".mml-home",
          transform(prefix, selector) {
            if (selector === ":root" || selector === "html" || selector === "body") return prefix;
            if (selector === '[data-mml-theme="Light"]') return prefix + selector;
            return prefix + " " + selector;
          },
        }),
      ],
    },
  },
  build: {
    lib: {
      entry: "src/entry.ts",
      formats: ["iife"],
      name: "MmlCustomHome",
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
