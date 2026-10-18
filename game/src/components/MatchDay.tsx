import { useEffect, useState } from 'react'
import type { ClubRef } from '../bindings/ClubRef'
import type { Fixture } from '../bindings/Fixture'

const MATCH_MS = 2600 // o relógio vai de 0' a 90' neste tempo
const AFTER_MS = 1600 // e o resultado fica à vista antes de ir para os Resultados

function Team({ club, align }: { club: ClubRef; align: 'left' | 'right' }) {
  const style = club.fg && club.bg ? { color: club.fg, background: club.bg } : { color: '#fff', background: '#3a3a3a' }
  return (
    <div
      className={`flex h-14 flex-1 items-center border-2 border-black px-4 font-title text-[24px] font-bold ${align === 'right' ? 'justify-end' : ''}`}
      style={style}
    >
      {club.shortName}
    </div>
  )
}

/**
 * Dia de jogo do treinador: os dois clubes, o relógio a correr e o resultado no fim; depois
 * vai sozinho para os Resultados da jornada (um clique salta a espera). O relato lance a
 * lance chega no marco 3.4, com o motor por eventos.
 */
export default function MatchDay({ fixture, onDone }: { fixture: Fixture; onDone: () => void }) {
  const [minute, setMinute] = useState(0)
  const finished = minute >= 90

  useEffect(() => {
    const start = performance.now()
    const tick = setInterval(() => {
      const m = Math.min(90, Math.floor(((performance.now() - start) / MATCH_MS) * 90))
      setMinute(m)
      if (m >= 90) clearInterval(tick)
    }, 40)
    return () => clearInterval(tick)
  }, [])

  useEffect(() => {
    if (!finished) return
    const t = setTimeout(onDone, AFTER_MS)
    return () => clearTimeout(t)
  }, [finished, onDone])

  return (
    <div className="absolute inset-0 z-50 flex cursor-pointer items-center justify-center bg-black/70" onClick={onDone}>
      <div className="w-[640px] max-w-full border-2 border-[#1d4a20] bg-[#0b3a10] p-2 shadow-[4px_4px_0_rgba(0,0,0,0.6)]">
        <div className="flex h-11 items-center justify-center border-2 border-[#333] bg-black font-title text-xl text-white">
          Dia de Jogo
        </div>
        <div className="py-2 text-center text-cm-yellow">
          {fixture.competition} · {fixture.round}ª jornada
        </div>
        <div className="flex items-center gap-2 px-1">
          <Team club={fixture.home} align="right" />
          <div className="flex h-14 w-28 shrink-0 items-center justify-center border-2 border-black bg-black font-title text-[30px] font-bold text-cm-yellow">
            {finished ? `${fixture.homeGoals ?? 0} - ${fixture.awayGoals ?? 0}` : '–'}
          </div>
          <Team club={fixture.away} align="left" />
        </div>
        <div className="mt-3 flex items-center gap-2 px-1">
          <span className="w-12 text-right font-title text-[20px] font-bold text-white">{minute}'</span>
          <div className="h-4 flex-1 border border-black bg-black p-0.5">
            <div className={`h-full ${finished ? 'bg-cm-yellow' : 'cm-progress'}`} style={{ width: `${(minute / 90) * 100}%` }} />
          </div>
        </div>
        <div className="py-2 text-center text-[13px] text-[#bfe6c4]">
          {finished ? 'Final do jogo · a abrir os resultados da jornada…' : 'A jogar… (clique para ver o resultado)'}
        </div>
      </div>
    </div>
  )
}
