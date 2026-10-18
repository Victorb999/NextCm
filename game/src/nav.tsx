import { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react'
import type { ReactNode } from 'react'

// Navegação entre telas com histórico, como os botões ◀ ▶ e Atrás/Seguinte do CM.

export type Screen =
  | { kind: 'search' }
  | { kind: 'person'; id: number }
  | { kind: 'club'; id: number }
  | { kind: 'competitions' }
  | { kind: 'competition'; id: number }
  | { kind: 'nations' }
  | { kind: 'nation'; id: number }

type Nav = {
  current: Screen
  go: (s: Screen) => void
  back: () => void
  forward: () => void
  canBack: boolean
  canForward: boolean
}

const NavContext = createContext<Nav | null>(null)

export function NavProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState<{ stack: Screen[]; index: number }>({ stack: [{ kind: 'search' }], index: 0 })

  const go = useCallback((s: Screen) => {
    setState(({ stack, index }) => {
      const cur = stack[index]
      if (JSON.stringify(cur) === JSON.stringify(s)) return { stack, index }
      // Ir para uma tela nova descarta o "avançar", como num navegador
      return { stack: [...stack.slice(0, index + 1), s], index: index + 1 }
    })
  }, [])
  const back = useCallback(() => setState((st) => ({ ...st, index: Math.max(0, st.index - 1) })), [])
  const forward = useCallback(
    () => setState((st) => ({ ...st, index: Math.min(st.stack.length - 1, st.index + 1) })),
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
