import { useEffect, useRef, useState } from 'react'
import type { ClubRef } from '../bindings/ClubRef'
import type { Fixture } from '../bindings/Fixture'
import type { StandingRow } from '../bindings/StandingRow'

const MATCH_MS = 2600 // o relógio vai de 0' a 90' neste tempo
const AFTER_MS = 1800 // e o resultado fica à vista antes de ir para os Resultados

/** Faixa de um clube no resultado, como no CM: nome grande nas cores e o placar numa casa. */
function ScoreBand({ club, goals, side }: { club: ClubRef; goals: number | null; side: 'home' | 'away' }) {
  const style = club.fg && club.bg ? { color: club.fg, background: club.bg } : { color: '#fff', background: '#3a3a3a' }
  const box = (
    <span className="flex h-11 w-12 shrink-0 items-center justify-center border-2 border-black bg-[#e8e8e8] font-title text-[30px] font-bold text-black">
      {goals ?? ''}
    </span>
  )
  return (
    <div className="flex h-14 flex-1 items-center gap-2 border-2 border-black px-2" style={style}>
      {side === 'away' && box}
      <span
        className="flex-1 truncate font-title text-[28px] font-bold leading-[38px]"
        style={{ textShadow: '2px 2px 0 rgba(0,0,0,0.35)' }}
      >
        {club.shortName}
      </span>
      {side === 'home' && box}
    </div>
  )
}

/** Jogos do dia na competição, como "Jogos da Tarde" / "Resultados da Tarde" do CM. */
function DayList({
  title,
  fixture,
  games,
  table,
  own,
  scores,
  onNext,
}: {
  title: string
  fixture: Fixture
  games: Fixture[]
  table: StandingRow[]
  own: number
  scores: boolean
  onNext: () => void
}) {
  const pos = (id: number) => {
    const r = table.find((r) => r.club.id === id)
    return r && r.played > 0 ? `${r.position}º` : ''
  }
  const name = (c: ClubRef) => (
    <span className={`flex-1 truncate font-title text-[21px] leading-[28px] ${c.id === own ? 'text-cm-yellow' : 'text-white'}`}>
      {c.shortName}
    </span>
  )
  return (
    <div className="absolute inset-0 z-50 flex flex-col bg-cm-screen">
      <div className="shrink-0 p-1.5 pb-1">
        <div
          className="flex h-14 items-center justify-center border-2 border-black bg-[#1e9cf0] font-title text-[30px] font-bold text-white"
          style={{ textShadow: '-1px -1px 0 #000, 1px -1px 0 #000, -1px 1px 0 #000, 1px 1px 0 #000, 2px 2px 0 #000' }}
        >
          {fixture.competition}
        </div>
      </div>
      <div className="mx-1.5 flex h-12 shrink-0 items-center justify-center bg-black/60 font-title text-[26px] font-bold text-cm-yellow">
        {title} · {fixture.round}ª jornada
      </div>
      <div className="min-h-0 flex-1 overflow-auto px-6 pt-4">
        {games.map((f) => (
          <div key={f.id} className="flex h-[30px] items-center gap-3">
            <span className="cm-cell h-[22px] w-11 shrink-0 text-center text-[13px] leading-[20px] text-white">{pos(f.home.id)}</span>
            {name(f.home)}
            <span className="w-16 shrink-0 text-center font-title text-[21px] font-bold text-cm-magenta">
              {scores && f.homeGoals !== null ? `${f.homeGoals}:${f.awayGoals}` : 'v'}
            </span>
            {name(f.away)}
            <span className="cm-cell h-[22px] w-11 shrink-0 text-center text-[13px] leading-[20px] text-white">{pos(f.away.id)}</span>
          </div>
        ))}
      </div>
      <div className="flex h-11 shrink-0 gap-0.5 p-1.5 pt-1">
        <button className="cm-purple flex-[2] font-title text-xl" disabled>
          Atrás
        </button>
        <button className="cm-purple flex-1 font-title text-xl" onClick={onNext}>
          Seguinte
        </button>
      </div>
    </div>
  )
}

/**
 * Dia de jogo do treinador, como no CM: os jogos do dia (com as posições de antes), o
 * relógio de 0' a 90' com o resultado nas faixas dos clubes e, no fim, os resultados do dia
 * com as posições já atualizadas. Seguinte, Enter ou o Continuar Jogo avançam cada fase.
 * O relato lance a lance chega no marco 3.4.
 */
export default function MatchDay({
  fixture,
  dayGames,
  table,
  tableAfter,
  own,
  step,
  onDone,
}: {
  fixture: Fixture
  /** Jogos da competição neste dia (o do treinador incluído), já com os resultados. */
  dayGames: Fixture[]
  /** Classificação antes e depois da jornada, para as posições. */
  table: StandingRow[]
  tableAfter: StandingRow[]
  own: number
  /** Sobe a cada Continuar Jogo na coluna: avança uma fase, como o Seguinte. */
  step?: number
  onDone: () => void
}) {
  const [phase, setPhase] = useState<'list' | 'play' | 'results'>('list')
  const [minute, setMinute] = useState(0)
  const finished = phase === 'play' && minute >= 90

  useEffect(() => {
    if (phase !== 'play') return
    const start = performance.now()
    const tick = setInterval(() => {
      const m = Math.min(90, Math.floor(((performance.now() - start) / MATCH_MS) * 90))
      setMinute((cur) => Math.max(cur, m))
      if (m >= 90) clearInterval(tick)
    }, 40)
    return () => clearInterval(tick)
  }, [phase])

  // Acabado o jogo, o resultado fica à vista um pouco e seguem os resultados do dia
  useEffect(() => {
    if (!finished) return
    const t = setTimeout(() => setPhase('results'), AFTER_MS)
    return () => clearTimeout(t)
  }, [finished])

  /** Seguinte: jogos → jogo; no jogo, o 1º clique mostra o resultado e o 2º os resultados do dia. */
  const next = () => {
    if (phase === 'list') setPhase('play')
    else if (phase === 'play') {
      if (minute < 90) setMinute(90)
      else setPhase('results')
    } else onDone()
  }
  const nextRef = useRef(next)
  useEffect(() => {
    nextRef.current = next
  })

  // O Continuar Jogo da coluna avança uma fase (só quando muda, não ao abrir)
  const firstStep = useRef(step)
  useEffect(() => {
    if (step === undefined || step === firstStep.current) return
    nextRef.current()
  }, [step])

  // Teclado: Enter/Espaço avançam, como o Seguinte
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Enter' && e.key !== ' ') return
      e.preventDefault()
      nextRef.current()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])

  if (phase === 'list') {
    return <DayList title="Jogos de Hoje" fixture={fixture} games={dayGames} table={table} own={own} scores={false} onNext={next} />
  }
  if (phase === 'results') {
    return <DayList title="Resultados de Hoje" fixture={fixture} games={dayGames} table={tableAfter} own={own} scores onNext={next} />
  }

  return (
    <div className="absolute inset-0 z-50 flex cursor-pointer flex-col bg-cm-screen" onClick={next}>
      <div className="flex shrink-0 gap-1 p-1.5">
        <ScoreBand club={fixture.home} goals={finished ? fixture.homeGoals : null} side="home" />
        <ScoreBand club={fixture.away} goals={finished ? fixture.awayGoals : null} side="away" />
      </div>
      <div className="flex flex-1 flex-col items-center justify-center gap-4 px-8">
        <div className="text-cm-yellow">
          {fixture.competition} · {fixture.round}ª jornada
        </div>
        <div className="flex w-full max-w-xl items-center gap-2">
          <span className="w-12 text-right font-title text-[24px] font-bold text-white">{minute}'</span>
          <div className="h-5 flex-1 border border-black bg-black p-0.5">
            <div className={`h-full ${finished ? 'bg-cm-yellow' : 'cm-progress'}`} style={{ width: `${(minute / 90) * 100}%` }} />
          </div>
        </div>
        <div className="text-[13px] text-[#bfe6c4]">
          {finished ? 'Final do jogo · clique para ver os resultados de hoje' : 'A jogar… (clique para ver o resultado)'}
        </div>
      </div>
    </div>
  )
}
