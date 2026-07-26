import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

// Le port est fixe : tauri.conf.json pointe dessus pour le dev.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: { port: 5183, strictPort: true },
  build: { target: 'chrome110', sourcemap: true },
})
