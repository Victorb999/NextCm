import { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react'
import type { ReactNode } from 'react'

// Navegação entre telas com histórico, como os botões ◀ ▶ e Atrás/Seguinte do CM.

export type Screen =
  | { kind: 'search' }
  | { kind: 'news' }
  | { kind: 'person'; id: number }
  | { kind: 'contract'; id: number }
  | { kind: 'squadNumbers' }
  | { kind: 'board' }
  | { kind: 'jobs' }
  | { kind: 'press'; id: number }
  | { kind: 'compare'; a: number; b: number }
  | { kind: 'worldTransfers' }
  | { kind: 'club'; id: number; reserves?: boolean }
  | { kind: 'tactics'; id: number }
  | { kind: 'clubHistory'; id: number }
  | { kind: 'competitions' }
  | { kind: 'competition'; id: number; tab?: string }
  | { kind: 'nations' }
  | { kind: 'nation'; id: number }

type Nav = {
  current: Screen
  go: (s: Screen) => void
  back: () => void
  forward: () => void
  canBack: boolean
  canForward: boolean
  /** Conta cada pedido de navegação (também para a tela onde já se está). */
  seq: number
}

const NavContext = createContext<Nav | null>(null)

export function NavProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState<{ stack: Screen[]; index: number; seq: number }>({
    stack: [{ kind: 'search' }],
    index: 0,
    seq: 0,
  })

  const go = useCallback((s: Screen) => {
    setState(({ stack, index, seq }) => {
      const cur = stack[index]
      if (JSON.stringify(cur) === JSON.stringify(s)) return { stack, index, seq: seq + 1 }
      // Ir para uma tela nova descarta o "avançar", como num navegador
      return { stack: [...stack.slice(0, index + 1), s], index: index + 1, seq: seq + 1 }
    })
  }, [])
  const back = useCallback(() => setState((st) => ({ ...st, index: Math.max(0, st.index - 1), seq: st.seq + 1 })), [])
  const forward = useCallback(
    () => setState((st) => ({ ...st, index: Math.min(st.stack.length - 1, st.index + 1), seq: st.seq + 1 })),
    [],
  )

  const value = useMemo(
    () => ({
      current: state.stack[state.index],
      go,
      back,
      forward,
      canBack: state.index > 0,
      canForward: state.index < state.stack.length - 1,
      seq: state.seq,
    }),
    [state, go, back, forward],
  )
  return <NavContext.Provider value={value}>{children}</NavContext.Provider>
}

// eslint-disable-next-line react-refresh/only-export-components
export function useNav() {
  const nav = useContext(NavContext)
  if (!nav) throw new Error('useNav fora do NavProvider')
  return nav
}

/** Esc / Backspace voltam para a tela anterior (fora de campos de texto). */
// eslint-disable-next-line react-refresh/only-export-components
export function useEscBack() {
  const nav = useNav()
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.key === 'Escape' || e.key === 'Backspace') && !(document.activeElement instanceof HTMLInputElement)) {
        nav.back()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [nav])
}
