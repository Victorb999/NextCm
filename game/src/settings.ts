import { useSyncExternalStore } from 'react'
import { photoUrl } from './backgrounds'

// "Definições do Jogo" (botão Opções Jogo): preferências de exibição guardadas no
// localStorage. Não fazem parte do save: valem para qualquer jogo neste computador.

export type Currency = 'EUR' | 'GBP' | 'USD' | 'BRL'
export type WagePeriod = 'week' | 'month' | 'year'
export type TitleFont =
  | 'futurista'
  | 'classica'
  | 'moderna'
  | 'googleSans'
  | 'roboto'
  | 'robotoCondensed'
  | 'lato'
  | 'oswald'
  | 'garamond'
  | 'dmSerif'
  | 'play'
  | 'shadows'
export type TextFont = 'tahoma' | 'googleSans' | 'roboto' | 'robotoCondensed' | 'lato' | 'play' | 'oswald'
export type ScreenMode = 'window' | 'fullscreen'
export type Background = 'default' | 'plain' | 'photo' | 'image'
export type MatchSpeed = 'veryFast' | 'fast' | 'normal' | 'slow' | 'verySlow'

export type Settings = {
  currency: Currency
  wages: WagePeriod
  /** Letra dos títulos e da letra do resto do texto. */
  font: TitleFont
  textFont: TextFont
  screen: ScreenMode
  /** Cor base da coluna esquerda (#rrggbb); os relevos saem dela. */
  sideColour: string
  background: Background
  /** Foto de fundo que vem com o jogo (chave de `BACKGROUND_PHOTOS`). */
  backgroundPhoto: string | null
  /** Imagem do fundo personalizado (data URL, já reduzida). */
  backgroundImage: string | null
  /** Quanto escurecer a imagem de fundo (0–0.8), para o texto continuar legível. */
  backgroundDim: number
  /** Velocidade do relato dos jogos. */
  matchSpeed: MatchSpeed
}

export const DEFAULT_SETTINGS: Settings = {
  currency: 'EUR',
  wages: 'week',
  font: 'futurista',
  textFont: 'tahoma',
  screen: 'window',
  sideColour: '#45526a',
  background: 'default',
  backgroundPhoto: null,
  backgroundImage: null,
  backgroundDim: 0.5,
  matchSpeed: 'normal',
}

/** "Velocidade Comentário" do CM: quanto fica cada frase do relato no ecrã. */
export const MATCH_SPEEDS: { key: MatchSpeed; label: string; ms: number }[] = [
  { key: 'veryFast', label: 'Muito Rápida', ms: 60 },
  { key: 'fast', label: 'Rápida', ms: 200 },
  { key: 'normal', label: 'Média', ms: 550 },
  { key: 'slow', label: 'Lenta', ms: 1100 },
  { key: 'verySlow', label: 'Muito Lenta', ms: 1900 },
]

/** Moedas como no CM; o banco está em euros. Câmbio aproximado do início da época 25/26. */
export const CURRENCIES: { key: Currency; label: string; symbol: string; rate: number }[] = [
  { key: 'EUR', label: '€uro', symbol: '€', rate: 1 },
  { key: 'GBP', label: 'Libra', symbol: '£', rate: 0.86 },
  { key: 'USD', label: 'Dólar', symbol: '$', rate: 1.17 },
  { key: 'BRL', label: 'Real', symbol: 'R$', rate: 6.3 },
]

export const WAGES: { key: WagePeriod; label: string; suffix: string; long: string; factor: number }[] = [
  { key: 'week', label: 'Semanais', suffix: 'p/s', long: 'por semana', factor: 1 },
  { key: 'month', label: 'Mensais', suffix: 'p/m', long: 'por mês', factor: 52 / 12 },
  { key: 'year', label: 'Anuais', suffix: 'p/a', long: 'por ano', factor: 52 },
]

export const FONTS: { key: TitleFont; label: string; family: string }[] = [
  { key: 'futurista', label: 'Futurista', family: '"Trebuchet MS", Tahoma, sans-serif' },
  { key: 'classica', label: 'Clássica', family: 'Tahoma, Verdana, sans-serif' },
  { key: 'moderna', label: 'Moderna', family: '"Segoe UI", Arial, sans-serif' },
  { key: 'googleSans', label: 'Google Sans', family: '"Google Sans", sans-serif' },
  { key: 'roboto', label: 'Roboto', family: 'Roboto, sans-serif' },
  { key: 'robotoCondensed', label: 'Roboto Condensed', family: '"Roboto Condensed", sans-serif' },
  { key: 'lato', label: 'Lato', family: 'Lato, sans-serif' },
  { key: 'oswald', label: 'Oswald', family: 'Oswald, sans-serif' },
  { key: 'garamond', label: 'EB Garamond', family: '"EB Garamond", serif' },
  { key: 'dmSerif', label: 'DM Serif Display', family: '"DM Serif Display", serif' },
  { key: 'play', label: 'Play', family: 'Play, sans-serif' },
  { key: 'shadows', label: 'Shadows Into Light', family: '"Shadows Into Light", cursive' },
]

/** Letra do texto (listas, tabelas, botões): só as que se leem bem em 13 px. */
export const TEXT_FONTS: { key: TextFont; label: string; family: string }[] = [
  { key: 'tahoma', label: 'Tahoma (CM)', family: 'Tahoma, Verdana, sans-serif' },
  { key: 'googleSans', label: 'Google Sans', family: '"Google Sans", Tahoma, sans-serif' },
  { key: 'roboto', label: 'Roboto', family: 'Roboto, Tahoma, sans-serif' },
  { key: 'robotoCondensed', label: 'Roboto Condensed', family: '"Roboto Condensed", Tahoma, sans-serif' },
  { key: 'lato', label: 'Lato', family: 'Lato, Tahoma, sans-serif' },
  { key: 'play', label: 'Play', family: 'Play, Tahoma, sans-serif' },
  { key: 'oswald', label: 'Oswald', family: 'Oswald, Tahoma, sans-serif' },
]

/** Cores prontas da coluna esquerda (a 1ª é a do tema Coalizão). */
export const SIDE_PRESETS: { label: string; colour: string }[] = [
  { label: 'Coalizão', colour: '#45526a' },
  { label: 'Azul CM', colour: '#24448a' },
  { label: 'Grafite', colour: '#434343' },
  { label: 'Verde', colour: '#2e5c3a' },
  { label: 'Vinho', colour: '#61283a' },
]

const KEY = 'nextcm.settings'

function load(): Settings {
  try {
    const raw = localStorage.getItem(KEY)
    return raw ? { ...DEFAULT_SETTINGS, ...JSON.parse(raw) } : DEFAULT_SETTINGS
  } catch {
    return DEFAULT_SETTINGS
  }
}

let current = load()
const listeners = new Set<() => void>()

export function getSettings() {
  return current
}

/** Grava e aplica. Devolve `false` se o localStorage recusou (ex.: imagem grande demais). */
export function saveSettings(next: Settings): boolean {
  current = next
  let saved = true
  try {
    localStorage.setItem(KEY, JSON.stringify(next))
  } catch {
    saved = false
  }
  applySettings(next)
  listeners.forEach((l) => l())
  return saved
}

export function useSettings() {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l)
      return () => listeners.delete(l)
    },
    () => current,
  )
}

/** Mistura `hex` com branco (t > 0) ou preto (t < 0). */
export function shade(hex: string, t: number) {
  const n = parseInt(hex.slice(1, 7), 16)
  const target = t > 0 ? 255 : 0
  const k = Math.abs(t)
  const ch = (v: number) => Math.round(v + (target - v) * k)
  const [r, g, b] = [ch(n >> 16), ch((n >> 8) & 255), ch(n & 255)]
  return `#${((1 << 24) | (r << 16) | (g << 8) | b).toString(16).slice(1)}`
}

/** Variáveis CSS do tema, fundo e modo de ecrã. */
export function applySettings(s: Settings) {
  const root = document.documentElement.style
  const base = /^#[0-9a-f]{6}$/i.test(s.sideColour) ? s.sideColour : DEFAULT_SETTINGS.sideColour
  root.setProperty('--cm-side', base)
  root.setProperty('--cm-side-top', shade(base, 0.12))
  root.setProperty('--cm-sidebtn', shade(base, -0.17))
  root.setProperty('--cm-sidelight', shade(base, 0.38))
  root.setProperty('--cm-sidedark', shade(base, -0.58))
  root.setProperty('--cm-side-mark', shade(base, 0.2))
  root.setProperty('--cm-title-font', (FONTS.find((f) => f.key === s.font) ?? FONTS[0]).family)
  root.setProperty('--cm-text-font', (TEXT_FONTS.find((f) => f.key === s.textFont) ?? TEXT_FONTS[0]).family)

  // A foto do jogo ou a imagem do utilizador; sem nenhuma, o fundo padrão
  const url = s.background === 'photo' ? photoUrl(s.backgroundPhoto) : s.background === 'image' ? s.backgroundImage : undefined
  const bg = (s.background === 'photo' || s.background === 'image') && !url ? 'default' : s.background
  document.documentElement.dataset.bg = bg === 'photo' ? 'image' : bg
  if (url) {
    const dim = `rgba(0,0,0,${s.backgroundDim})`
    root.setProperty('--cm-bg-image', `linear-gradient(${dim}, ${dim}), url("${url}")`)
  } else {
    root.removeProperty('--cm-bg-image')
  }
}

/** Entra/sai do ecrã completo (janela do Tauri ou, no navegador, a Fullscreen API). */
export async function applyScreenMode(mode: ScreenMode) {
  const full = mode === 'fullscreen'
  try {
    const { isTauri } = await import('@tauri-apps/api/core')
    if (isTauri()) {
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      await getCurrentWindow().setFullscreen(full)
      return
    }
    if (full && !document.fullscreenElement) await document.documentElement.requestFullscreen()
    if (!full && document.fullscreenElement) await document.exitFullscreen()
  } catch {
    // O navegador pode recusar sem um clique do utilizador; não é grave
  }
}

/** Lê uma imagem e reduz para no máximo 1920 px (JPEG), para caber no localStorage. */
export function loadBackgroundImage(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const url = URL.createObjectURL(file)
    const img = new Image()
    img.onload = () => {
      const scale = Math.min(1, 1920 / Math.max(img.width, img.height))
      const canvas = document.createElement('canvas')
      canvas.width = Math.round(img.width * scale)
      canvas.height = Math.round(img.height * scale)
      canvas.getContext('2d')!.drawImage(img, 0, 0, canvas.width, canvas.height)
      URL.revokeObjectURL(url)
      resolve(canvas.toDataURL('image/jpeg', 0.85))
    }
    img.onerror = () => {
      URL.revokeObjectURL(url)
      reject(new Error('Não foi possível ler a imagem.'))
    }
    img.src = url
  })
}
