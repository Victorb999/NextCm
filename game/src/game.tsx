import { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react'
import type { ReactNode } from 'react'
import { api, hasBackend } from './api'
import type { GameInfo } from './bindings/GameInfo'

// Estado do jogo em curso para toda a interface: data, treinador, clube, notícias por ler.
// `mode` diz o que a área principal mostra fora da navegação normal (menu inicial etc.).

export type Mode = 'loading' | 'start' | 'newGame' | 'loadGame' | 'explore' | 'game'

type GameCtx = {
  info: GameInfo | null
  mode: Mode
  setMode: (m: Mode) => void
  /** Guarda o estado devolvido por um comando do jogo e entra no modo jogo. */
  update: (info: GameInfo) => void
  /** Fecha o jogo (sem gravar) e volta ao menu inicial. */
  close: () => Promise<void>
}

const Ctx = createContext<GameCtx | null>(null)

export function GameProvider({ children }: { children: ReactNode }) {
  const [info, setInfo] = useState<GameInfo | null>(null)
  const [mode, setMode] = useState<Mode>('loading')

  // Ao abrir (ou recarregar a página em dev), retoma o jogo que o backend tiver aberto
  useEffect(() => {
    if (!hasBackend) return
    api.gameInfo().then(
      (g) => {
        setInfo(g)
        setMode(g ? 'game' : 'start')
      },
      () => setMode('start'),
    )
  }, [])

  const update = useCallback((g: GameInfo) => {
    setInfo(g)
    setMode('game')
  }, [])

  const close = useCallback(async () => {
    await api.closeGame()
    setInfo(null)
    setMode('start')
  }, [])

  const value = useMemo(() => ({ info, mode, setMode, update, close }), [info, mode, update, close])
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>
}

// eslint-disable-next-line react-refresh/only-export-components
export function useGame() {
  const g = useContext(Ctx)
  if (!g) throw new Error('useGame fora do GameProvider')
  return g
}
