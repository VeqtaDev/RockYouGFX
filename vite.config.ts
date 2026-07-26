import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import pkg from './package.json' with { type: 'json' }

// Le port est fixe : tauri.conf.json pointe dessus pour le dev.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: { port: 5183, strictPort: true },
  build: { target: 'chrome110', sourcemap: true },
  define: {
    // La vérification de mise à jour compare cette valeur au dernier tag
    // publié : elle doit venir d'une source unique, d'où le package.json.
    __APP_VERSION__: JSON.stringify(pkg.version),
  },
})
