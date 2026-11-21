import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { fileURLToPath, URL } from 'node:url';

const r = (p: string) => fileURLToPath(new URL(p, import.meta.url));

// 每种窗口一个入口：settings（设置），后续加入 overlay / pin / card。
export default defineConfig({
  root: r('./src/entries'),
  publicDir: false,
  plugins: [svelte()],
  resolve: {
    alias: { $lib: r('./src/lib') },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ['**/src-tauri/**', '**/target/**'] },
  },
  build: {
    outDir: r('./dist'),
    emptyOutDir: true,
    target: 'es2022',
    rollupOptions: {
      input: {
        settings: r('./src/entries/settings/index.html'),
      },
    },
  },
  test: {
    root: r('.'),
    include: ['src/**/*.test.ts'],
    environment: 'node',
  },
});
