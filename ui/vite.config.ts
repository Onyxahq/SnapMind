import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { fileURLToPath, URL } from 'node:url';

const r = (p: string) => fileURLToPath(new URL(p, import.meta.url));

// 每种窗口一个入口：settings（设置）、overlay（截图遮罩）、pin（贴图）、pintools（贴图工具条），后续加入 card。
export default defineConfig({
  root: r('./src/entries'),
  publicDir: false,
  plugins: [svelte({ configFile: false })],
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
        overlay: r('./src/entries/overlay/index.html'),
        pin: r('./src/entries/pin/index.html'),
        pintools: r('./src/entries/pintools/index.html'),
        selcard: r('./src/entries/selcard/index.html'),
      },
    },
  },
  test: {
    root: r('..'),
    include: ['ui/src/**/*.test.ts', 'scripts/**/*.test.ts'],
    environment: 'node',
  },
});
