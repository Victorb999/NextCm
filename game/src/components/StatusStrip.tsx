import { useEffect, useState } from 'react'
import { api } from '../api'
import type { StatusBar } from '../bindings/StatusBar'
import { useGame } from '../game'
import { type Screen, useNav } from '../nav'

// O meio da barra de baixo (marco 4.0, como a do FM08): o caminho da tela aberta (Europa › Inglaterra ›
// Premier › Man Utd, cada passo abre a tela dele) e, por baixo, os resultados do último dia com jogos,
// um de cada vez. Sem animação: o resultado troca a cada poucos segundos.

const EVERY_MS = 3500

let lastBar: StatusBar | null = null

function screenId(s: Screen): number | null {
  return 'id' in s && typeof s.id === 'number' ? s.id : null
}

function target(kind: string, id: number): Screen | null {
  switch (kind) {
    case 'nation':
    case 'competition':
    case 'club':
    case 'person':
      return { kind, id }
    default:
      return null
  }
}

export default function StatusStrip() {
  const nav = useNav()
  const game = useGame()
  const date = game.mode === 'game' ? game.info?.date : undefined
  // Numa tela nova, a barra da anterior fica até chegar a dela (o Atrás não muda de largura)
  const [bar, setBar] = useState<StatusBar | null>(lastBar)
  const [tick, setTick] = useState(0)
  const kind = nav.current.kind
  const id = screenId(nav.current)

  useEffect(() => {
    if (!date) return
    let live = true
    api.statusBar(kind, id).then(
      (b) => {
        lastBar = b
        if (live) setBar(b)
      },
      () => {},
    )
    return () => {
      live = false
    }
  }, [kind, id, date])

  const count = bar?.results.length ?? 0
  useEffect(() => {
    if (count < 2) return
    const t = window.setInterval(() => setTick((i) => i + 1), EVERY_MS)
    return () => window.clearInterval(t)
  }, [count])
  const at = count ? tick % count : 0

  if (!date || !bar) return null
  return (
    <div className="flex min-w-0 flex-[3] flex-col justify-center rounded-[4px] bg-black/45 px-3 text-[13px] leading-[17px]">
      <div className="truncate text-[#c8c8c8]">
        {bar.path.map((p, i) => {
          const to = p.kind && p.id != null ? target(p.kind, p.id) : null
          const last = i === bar.path.length - 1
          return (
            <span key={i}>
              {i > 0 && <span className="px-1 text-cm-purplelight">›</span>}
              {to && !last ? (
                <button className="hover:text-cm-yellow hover:underline" onClick={() => nav.go(to)}>
                  {p.label}
                </button>
              ) : (
                <span className={last ? 'font-bold text-white' : ''}>{p.label}</span>
              )}
            </span>
          )
        })}
      </div>
      <div className="truncate text-cm-yellow tabular-nums" title={bar.results.join('\n')}>
        {bar.results[at] ?? <span className="text-[#8a8a8a]">Sem resultados ainda</span>}
      </div>
    </div>
  )
}
