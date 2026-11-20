import { defineConfig } from 'vite';
import { fileURLToPath } from 'node:url';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: { alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) } },
  build: { assetsInlineLimit: 0 },
  clearScreen: false,
  server: { port: 1420, strictPort: true, host: '127.0.0.1' },
  envPrefix: ['VITE_'],
});
