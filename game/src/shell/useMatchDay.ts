import { useCallback, useEffect, useRef, useState } from 'react'
import { useNav } from '../nav'
import type { MatchDayData } from './types'

/**
 * O dia de jogo do treinador por cima das telas: aberto quando o Continuar Jogo chega ao jogo,
 * posto em espera (escondido) quando se navega pela coluna, e de volta com o Continuar Jogo,
 * que com ele aberto faz o papel do Seguinte (`step` conta as fases).
 */
export function useMatchDay() {
  const nav = useNav()
  const [match, setMatch] = useState<MatchDayData | null>(null)
  const [step, setStep] = useState(0)
  const [hidden, setHidden] = useState(false)
  // A navegação quando o dia de jogo foi aberto: qualquer navegação depois (mesmo para a
  // tela onde já se estava) esconde-o
  const screen = useRef<number | null>(null)

  useEffect(() => {
    if (!match) {
      screen.current = null
      return
    }
    if (screen.current === null) screen.current = nav.seq
    else if (screen.current !== nav.seq) setHidden(true)
  }, [match, nav.seq])

  const close = useCallback(() => {
    // Depois dos resultados do dia, como no CM: a caixa de mensagens (com a notícia do jogo)
    if (match) nav.go({ kind: 'news' })
    setMatch(null)
    setHidden(false)
  }, [match, nav])

  /** O Continuar Jogo com o dia de jogo aberto: volta a ele ou avança uma fase. */
  const advance = match
    ? () => {
        if (hidden) {
          screen.current = nav.seq
          setHidden(false)
        } else setStep((n) => n + 1)
      }
    : undefined

  return { match, open: setMatch, close, step, hidden, advance }
}
