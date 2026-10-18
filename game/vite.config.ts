import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  // Ponte de dev para abrir a UI no navegador (core/examples/dev_bridge.rs)
  server: { proxy: { '/invoke': 'http://127.0.0.1:1421' } },
})
