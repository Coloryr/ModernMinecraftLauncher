import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

// 1421 与启动器的 1420 错开，两个程序可以同时开着。
export default defineConfig({
  base: './',
  plugins: [vue()],
  server: { host: '127.0.0.1', port: 1421, strictPort: true },
  build: { chunkSizeWarningLimit: 3000 },
});
