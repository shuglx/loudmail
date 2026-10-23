import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = dirname(fileURLToPath(import.meta.url));

// 目标机 WebKitGTK 2.28（约 Safari 13 时代引擎），构建目标锁定 es2020/safari13
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  build: {
    target: ['es2020', 'safari13'],
    outDir: 'dist',
    rollupOptions: {
      input: resolve(root, 'index.html'),
    },
  },
  server: {
    port: 5173,
    strictPort: true,
    // 不监听 Rust 产物目录：Windows 上正在运行的 loudmail.exe 被 fs.watch 打开会抛
    // EBUSY 导致 vite 崩掉，进而 tauri dev 报 "beforeDevCommand terminated"
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
});
