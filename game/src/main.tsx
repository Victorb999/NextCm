import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './assets/fonts/fonts.css'
import './index.css'
import App from './App.tsx'
import { applyScreenMode, applySettings, getSettings } from './settings'

// Tema, fundo e ecrã guardados em Opções Jogo, antes do 1º desenho
applySettings(getSettings())
if (getSettings().screen === 'fullscreen') void applyScreenMode('fullscreen')

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
