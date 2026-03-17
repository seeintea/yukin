import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { tanstackRouter } from '@tanstack/router-plugin/vite'
import electron from 'vite-plugin-electron'
import path from 'path'

const isElectron = process.env.ELECTRON === 'true'

export default defineConfig({
  plugins: [
    tanstackRouter(),
    react(),
    ...(isElectron ? [
      electron([
        {
          entry: 'src/main.ts',
          onstart: (options) => options.startup(),
          vite: {
            build: {
              sourcemap: true,
              minify: false,
              outDir: 'dist-electron/main',
            },
          },
        },
        {
          entry: 'src/preload.ts',
          onstart: (options) => options.reload(),
          vite: {
            build: {
              sourcemap: true,
              minify: false,
              outDir: 'dist-electron/preload',
            },
          },
        },
      ]),
    ] : []),
  ],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  base: './',
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
})
