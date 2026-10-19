import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// 整页自定义界面示例工程
//
// 产物（dist/）由启动器的自定义界面协议直接提供，资源一律用相对路径（base: "./"）。
export default defineConfig({
  plugins: [vue()],
  base: "./",
  server: {
    // 只在浏览器里预览用（无 IPC 数据）；避开 mml-vue 的 1420 与自定义主页的 1520
    port: 1522,
    strictPort: true,
  },
});
