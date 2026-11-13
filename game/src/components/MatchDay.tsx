import { useEffect, useRef, useState } from 'react'
import { api } from '../api'
import type { Fixture } from '../bindings/Fixture'
import type { StandingRow } from '../bindings/StandingRow'
import { useGame } from '../game'
import type { MatchPhase } from '../shell/useMatchDay'
import PreMatch from './PreMatch'
import { DayList } from './match/DayList'
import { LiveScreen } from './match/LiveScreen'

/**
 * Dia de jogo do treinador, como no CM: os jogos do dia (com as posições de antes), a
 * partida ao vivo e, no fim, os resultados do dia com as posições já atualizadas.
 * Seguinte, Enter ou o Continuar Jogo avançam cada fase.
 */
export default function MatchDay({
  fixture,
  dayGames,
  table,
  own,
  step,
  active = true,
  onDone,
  onPhase,
}: {
  fixture: Fixture
  /** Jogos da competição neste dia (o do treinador incluído), ainda por jogar. */
  dayGames: Fixture[]
  /** Classificação antes da jornada, para as posições. */
  table: StandingRow[]
  own: number
  /** Sobe a cada Continuar Jogo na coluna: faz a acção principal, como o Enter. */
  step?: number
  /** Escondido enquanto o treinador navega: fica montado (guarda a fase) mas não reage. */
  active?: boolean
  onDone: () => void
  /** A fase do dia, a cada mudança. */
  onPhase?: (p: MatchPhase) => void
}) {
  const game = useGame()
  const activeRef = useRef(active)
  useEffect(() => {
    activeRef.current = active
  })
  // Os jogos da tarde já acabados (o jogo do treinador é à noite): primeiro os resultados deles
  const earlier = dayGames.filter((f) => f.homeGoals !== null)
  const pending = dayGames.filter((f) => f.homeGoals === null)
  const night = game.mode === 'game' && game.info?.part === 2
  const [phase, setPhase] = useState<MatchPhase>(earlier.length > 0 ? 'earlier' : 'list')
  // O Shell usa a fase para o rótulo do Continuar Jogo e para trancar a coluna
  useEffect(() => {
    onPhase?.(phase)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [phase])
  const [results, setResults] = useState<{
    games: Fixture[]
    table: StandingRow[]
  } | null>(null)
  // Depois dos resultados da competição do treinador, os das outras que jogaram hoje
  const [others, setOthers] = useState<number[]>([])
  const live = useRef<{ main: () => void }>(null)

  /** Acaba o jogo no core (resultados, notícias) e mostra os resultados do dia. */
  const finish = async () => {
    try {
      game.update(await api.liveFinish())
      const [all, tableAfter] = await Promise.all([api.listFixtures(fixture.competitionId), api.leagueTable(fixture.competitionId)])
      // Só os já jogados: os da noite (o jogo do treinador foi à tarde) ainda não têm resultado
      setResults({
        games: all.filter((f) => f.date === fixture.date && f.homeGoals !== null),
        table: tableAfter,
      })
      setPhase('results')
      api.dayCompetitions().then(
        (c) => setOthers(c.filter((id) => id !== fixture.competitionId)),
        () => setOthers([]),
      )
    } catch (e) {
      console.error(e)
      onDone()
    }
  }

  /** Os resultados de hoje da próxima competição (como os "Resultados da Noite" do CM). */
  const nextCompetition = async () => {
    const [id, ...rest] = others
    setOthers(rest)
    if (id === undefined) return onDone()
    try {
      const [all, table] = await Promise.all([api.listFixtures(id), api.leagueTable(id)])
      const games = all.filter((f) => f.date === fixture.date && f.homeGoals !== null)
      if (games.length === 0) return rest.length ? void nextCompetition() : onDone()
      setResults({ games, table })
    } catch {
      onDone()
    }
  }

  const next = () => {
    if (phase === 'earlier') setPhase('list')
    else if (phase === 'list') setPhase('pre')
    // O Continuar Jogo no Pré-Jogo passa pela Palestra e depois vai ao jogo
    else if (phase === 'pre') {
      if (pre.current) pre.current.main()
      else setPhase('play')
    } else if (phase === 'play') live.current?.main()
    else if (others.length > 0) void nextCompetition()
    else onDone()
  }
  const pre = useRef<{ main: () => void } | null>(null)
  const nextRef = useRef(next)
  useEffect(() => {
    nextRef.current = next
  })

  // O Continuar Jogo da coluna faz a acção principal (só quando muda, não ao abrir)
  const firstStep = useRef(step)
  useEffect(() => {
    if (step === undefined || step === firstStep.current) return
    nextRef.current()
  }, [step])

  // Teclado: Enter/Espaço avançam (na partida: iniciar, pausar, retomar)
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!activeRef.current || (e.key !== 'Enter' && e.key !== ' ')) return
      e.preventDefault()
      nextRef.current()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])

  if (phase === 'earlier') {
    return <DayList title="Resultados da Tarde" fixture={fixture} games={earlier} table={table} own={own} scores onNext={next} />
  }
  if (phase === 'list') {
    return (
      <DayList
        title={night ? 'Jogos da Noite' : 'Jogos da Tarde'}
        fixture={fixture}
        games={pending}
        table={table}
        own={own}
        scores={false}
        onNext={next}
      />
    )
  }
  if (phase === 'pre') return <PreMatch ref={pre} onPlay={() => setPhase('play')} />
  if (phase === 'results' && results) {
    return (
      <DayList
        title={night ? 'Resultados da Noite' : 'Resultados da Tarde'}
        fixture={results.games[0] ?? fixture}
        games={results.games}
        table={results.table}
        own={own}
        scores
        onNext={next}
      />
    )
  }
  return <LiveScreen ref={live} own={own} active={active} onFinished={() => void finish()} />
}
