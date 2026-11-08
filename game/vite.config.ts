import { readFileSync } from 'node:fs'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

// A versão mostrada no menu vem do tauri.conf.json (a mesma do instalador)
const version: string = JSON.parse(readFileSync(new URL('./src-tauri/tauri.conf.json', import.meta.url), 'utf-8')).version

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  define: { __APP_VERSION__: JSON.stringify(version) },
  // Ponte de dev para abrir a UI no navegador (core/examples/dev_bridge.rs)
  // NEXTCM_BRIDGE: a porta da ponte (por omissão 1421), para correr dois ao mesmo tempo
  server: { proxy: { '/invoke': `http://127.0.0.1:${process.env.NEXTCM_BRIDGE ?? '1421'}` } },
})
