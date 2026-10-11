import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// https://vite.dev/config/
export default defineConfig({
  plugins: [vue()],
  server: {
    // 与 mml-gui 里 Tauri 的 devUrl 保持一致；不使用 Tauri 时可自行修改
    port: 1420,
    strictPort: true,
  },
  build: {
    // 默认阈值 500 kB 对本项目会**每次构建都刷一条已知的无用警告**：
    // 只有 LogWindow 那一块超，即 Monaco 编辑器，约 2.6 MB；它由
    // `defineAsyncComponent` 按需加载，只在打开日志窗口时才下载。
    //
    // 这里抬高到刚好高于 Monaco 的量级，而不是直接关掉警告：将来若有**新的**
    // 大块超出这个量级，比如误把某个重依赖静态引进了主窗口，构建仍会提醒。
    chunkSizeWarningLimit: 3000,
  },
});
