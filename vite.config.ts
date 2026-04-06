import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'path'

const isWebBuild = process.env.VITE_BUILD_TARGET === 'web'

export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
  ],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    allowedHosts: true,
    proxy: isWebBuild
      ? {
          '/api': {
            target: 'http://127.0.0.1:8080',
            changeOrigin: true,
          },
          '/health': {
            target: 'http://127.0.0.1:8080',
            changeOrigin: true,
          },
          '/ws': {
            target: 'ws://127.0.0.1:8080',
            ws: true,
            configure: (proxy) => {
              proxy.on('error', () => {});
            },
          },
        }
      : undefined,
  },
  resolve: isWebBuild
    ? {
        alias: {
          '@tauri-apps/api/core': path.resolve(__dirname, 'src/lib/stubs/tauri-core.ts'),
          '@tauri-apps/api/event': path.resolve(__dirname, 'src/lib/stubs/tauri-event.ts'),
          '@tauri-apps/plugin-dialog': path.resolve(__dirname, 'src/lib/stubs/tauri-dialog.ts'),
          '@tauri-apps/plugin-opener': path.resolve(__dirname, 'src/lib/stubs/tauri-opener.ts'),
          '@tauri-apps/plugin-fs': path.resolve(__dirname, 'src/lib/stubs/tauri-fs.ts'),
        },
      }
    : undefined,
})
