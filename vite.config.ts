import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: { alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) } },
  // Tauri から開くとき用に、ポートを固定する
  // src-tauri/ と core/ のビルド結果(target/)が変わるたびに見張らないようにする
  server: { port: 5173, strictPort: true, watch: { ignored: ['**/src-tauri/**', '**/core/**'] } },
  clearScreen: false,
})
