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
  },
});
