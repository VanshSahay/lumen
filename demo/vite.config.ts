import { defineConfig } from 'vite';

export default defineConfig({
  server: {
    port: 3000,
    open: true,
    // Allow serving WASM files from the packages directory
    fs: {
      allow: ['..'],
    },
  },
  build: {
    target: 'esnext',
  },
  optimizeDeps: {
    exclude: ['lumen-eth'],
  },
  worker: {
    format: 'es',
  },
});
