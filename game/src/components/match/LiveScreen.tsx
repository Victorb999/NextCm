import { Landmark, MessageSquare, Pause, Play, SlidersHorizontal } from 'lucide-react'
import { useEffect, useImperativeHandle, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import { createPortal } from 'react-dom'
import { api } from '../../api'
import type { LiveView } from '../../bindings/LiveView'
import * as sound from '../../sound'
import { MATCH_SPEEDS, getSettings, saveSettings, useSettings } from '../../settings'
import LiveTactics from '../LiveTactics'
import { AnalysisPane, PostMatchPane } from '../PreMatch'
import TalkPanel from '../TalkPanel'
import { clubBand, isStoppage, isGoalLine, manOfTheMatch, matchTab } from './lines'
import type { Line, Pane } from './lines'
import { ScoreBand } from './ScoreBand'
import { Panorama } from './Panorama'
import { Stats, Zones, Report, TeamData, Ratings, Results, Table, Highlights, History } from './MatchPanes'

function MatchSideButton({
  children,
  onClick,
  disabled,
  anchor,
  className = '',
  style,
}: {
  children: ReactNode
  onClick: (e: React.MouseEvent<HTMLButtonElement>) => void
  disabled?: boolean
  anchor?: boolean
  className?: string
  style?: React.CSSProperties
}) {
  return (
    <button
      className={`flex min-h-[40px] w-full items-center justify-center gap-2 rounded px-2 text-center font-bold leading-tight disabled:opacity-50 ${
        style ? '' : 'bg-cm-raised text-white hover:bg-cm-hover disabled:hover:bg-cm-raised'
      } ${className}`}
      style={style}
      disabled={disabled}
      onClick={onClick}
      data-menu-anchor={anchor || undefined}
    >
      {children}
    </button>
  )
}

/**
 * A partida ao vivo, como no CM: uma coluna própria (minuto, Iniciar/Pausa/Retomar,
 * Tácticas de cada clube, Velocidade Comentário) e, à direita, as faixas com o placar, as
 * abas Panorama Jogo / Estatísticas Jogo / Zonas Acção / Relatório Jogo e a barra de
 * baixo (Dados de cada clube, Classificações Jogadores, Últimos Resultados, Classificação
 * Geral). O jogo só começa no Iniciar Jogo e pára sozinho no intervalo.
 */
export function LiveScreen({
  ref,
  own,
  active,
  onFinished,
}: {
  /** A acção principal (Enter, Continuar Jogo da coluna), para o pai a chamar. */
  ref: React.Ref<{ main: () => void }>
  own: number
  /** Escondido (o treinador foi a outra tela): o relógio pára até voltar. */
  active: boolean
  onFinished: () => void
}) {
  const [view, setView] = useState<LiveView | null>(null)
  const [running, setRunning] = useState(false)
  const [pane, setPane] = useState<Pane>('panorama')
  const [tactics, setTactics] = useState<0 | 1 | null>(null)
  // A palestra do treinador (antes do jogo, ao intervalo, no fim)
  const [talk, setTalk] = useState(false)
  // Tácticas pedidas com o jogo a correr: abrem quando a bola parar (minuto do pedido)
  const [waiting, setWaiting] = useState<{
    side: 0 | 1
    since: number
    from: number
  } | null>(null)
  const resumeAfter = useRef(false)
  // O aviso das Tácticas abertas por uma lesão
  const [injuryNote, setInjuryNote] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [finishing, setFinishing] = useState(false)
  const [lines, setLines] = useState<Line[]>([])
  const [shown, setShown] = useState(0)
  // No fim, depois de o relato contar o apito final, abre o Pós-Jogo (uma vez)
  const postShown = useRef(false)
  const finishedSaid = !!view?.finished && shown >= lines.length
  useEffect(() => {
    if (finishedSaid && !postShown.current) {
      postShown.current = true
      setPane('post')
    }
  }, [finishedSaid])
  const [stepping, setStepping] = useState(false)
  const settings = useSettings()
  const speed = MATCH_SPEEDS.find((s) => s.key === settings.matchSpeed) ?? MATCH_SPEEDS[2] ?? MATCH_SPEEDS[0]

  const accept = (v: LiveView) => {
    setView(v)
    // Todas as frases do relato, pela ordem; as novas vão aparecendo uma a uma
    setLines(
      v.events.flatMap((e) =>
        e.lines.map((text, i) => ({
          minute: e.minute,
          side: e.side,
          text,
          kind: e.kind,
          playerId: e.playerId,
          last: i === e.lines.length - 1,
        })),
      ),
    )
  }

  useEffect(() => {
    api.liveMatch().then(
      (v) => {
        accept(v)
        // Ao voltar a um jogo a meio (página recarregada), o relato já dito não se repete
        setShown(v.events.reduce((n, e) => n + e.lines.length, 0))
      },
      (e) => {
        // Sem jogo ao vivo (o dia já acabou no core): passa aos resultados em vez de ficar preso
        setError(String(e))
        onFinished()
      },
    )
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // De volta ao dia de jogo antes do apito: a equipa pode ter mudado no Plantel ou nas Tácticas
  const kickedOff = !!view && view.minute > 0
  useEffect(() => {
    if (active && !kickedOff) api.liveMatch().then(accept, () => {})
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active])

  // O relato e o relógio: primeiro mostra as frases por dizer, uma de cada vez; com
  // tudo dito e o jogo a correr, joga o minuto seguinte
  useEffect(() => {
    if (!view || finishing || !active) return
    if (shown < lines.length) {
      const t = setTimeout(
        () => {
          setShown(shown + 1)
          const l = lines[shown]
          if (!l) return
          // Um lesionado da equipa do treinador: o jogo pára nas Tácticas, com o aviso (como no CM)
          if (l.kind === 'injury' && l.last && l.side !== null && l.side === view.userSide && !view.finished) {
            const team = l.side === 0 ? view.home : view.away
            const p = [...team.players, ...team.out].find((p) => p.id === l.playerId)
            setInjuryNote(
              `${p ? p.listName.split(', ').reverse().join(' ') : 'Um jogador'} está lesionado e não pode continuar. Faça a substituição.`,
            )
            openWaiting(l.side as 0 | 1)
          }
          // Tácticas pedidas: abrem na primeira paragem depois do pedido
          else if (waiting && shown + 1 > waiting.from && isStoppage(l)) openWaiting(waiting.side)
        },
        shown === 0 ? 0 : speed.ms,
      )
      return () => clearTimeout(t)
    }
    if (!running || view.finished || stepping || tactics !== null) return
    const t = setTimeout(
      () => {
        setStepping(true)
        api
          .liveStep(view.minute + 1)
          .then(
            (v) => {
              accept(v)
              if (v.halfTime || v.finished) setRunning(false)
              // Tácticas pedidas: o intervalo abre-as (sem retomar ao sair); o fim cancela-as
              if (waiting && v.finished) setWaiting(null)
              else if (waiting && v.halfTime) {
                resumeAfter.current = false
                setTactics(waiting.side)
                setWaiting(null)
              }
              // Sem nenhuma paragem em 3 minutos, as Tácticas abrem na mesma
              else if (waiting && v.minute - waiting.since >= 3) openWaiting(waiting.side)
            },
            (e) => {
              setError(String(e))
              setRunning(false)
            },
          )
          .finally(() => setStepping(false))
      },
      Math.max(40, speed.ms / 2),
    )
    return () => clearTimeout(t)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [view, lines, shown, running, stepping, tactics, finishing, speed.ms, waiting, active])

  // Os sons da partida: a torcida com o jogo a correr, e cada frase nova dita (não as de
  // um jogo retomado a meio) com o seu som
  const crowdOn = active && running && settings.sound && !!view && !view.finished && !view.halfTime
  useEffect(() => {
    if (crowdOn) sound.crowdStart()
    else sound.crowdStop()
  }, [crowdOn])
  useEffect(() => () => sound.crowdStop(), [])
  const heard = useRef<number | null>(null)
  useEffect(() => {
    const before = heard.current
    heard.current = shown
    if (before === null || shown !== before + 1 || !active) return
    const l = lines[shown - 1]
    if (!l) return
    if (isGoalLine(l)) sound.goal(l.side === 0)
    else if (l.kind === 'start') sound.whistleShort()
    else if (l.kind === 'half') sound.whistleShort()
    else if (l.kind === 'end' && l.playerId === null) sound.whistleEnd()
    else if (l.kind === 'goal' || l.kind === 'save' || l.kind === 'miss') sound.crowdSurge()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [shown])

  // Ao intervalo, com o relato dito, a palestra abre sozinha (uma vez; depois das Tácticas,
  // se o intervalo as abriu)
  const endTalk = useRef(false)
  const halfTalk = useRef(false)
  const atHalf = !!view?.halfTime && shown >= lines.length
  useEffect(() => {
    if (!atHalf) {
      halfTalk.current = false
      return
    }
    if (halfTalk.current || tactics !== null || !view || view.userSide === null) return
    halfTalk.current = true
    setTalk(true)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [atHalf, tactics])

  /** Abre as Tácticas pedidas com o jogo a correr; ao sair delas, o jogo continua. */
  function openWaiting(side: 0 | 1) {
    resumeAfter.current = true
    setRunning(false)
    setTactics(side)
    setWaiting(null)
  }

  const main = () => {
    // Na palestra, o Continuar Jogo sai da sala: antes do apito fica por aí; ao intervalo
    // recomeça o jogo e no fim segue para os resultados
    if (talk) {
      setTalk(false)
      if (view?.finished) endTalk.current = true
      if (!view || (!view.finished && !view.halfTime)) return
    }
    if (!view || finishing || tactics !== null) return
    if (view.finished) {
      if (shown < lines.length) return setShown(lines.length)
      // No fim, o Continuar abre primeiro a palestra (uma vez)
      if (!endTalk.current && view.userSide !== null) {
        endTalk.current = true
        return setTalk(true)
      }
      setFinishing(true)
      onFinished()
    } else if (waiting) {
      // Pausar com Tácticas pedidas abre-as já
      openWaiting(waiting.side)
      resumeAfter.current = false
    } else setRunning((r) => !r)
  }
  useImperativeHandle(ref, () => ({ main }))

  if (!view) {
    return <div className="cm-backdrop absolute inset-0 z-50 p-6 text-cm-red">{error}</div>
  }
  const started = view.minute > 0
  const label = view.finished
    ? 'Continuar Jogo'
    : running
      ? 'Pausa'
      : !started
        ? 'Iniciar Jogo'
        : view.halfTime
          ? 'Recomeçar Jogo'
          : 'Retomar Jogo'
  const current = shown > 0 ? (lines[Math.min(shown, lines.length) - 1] ?? null) : null
  const said = lines.slice(0, shown)
  // O placar só muda quando o relato chega ao golo
  const goalsSaid = (side: number) => said.filter((l) => isGoalLine(l) && l.side === side).length
  const locked = running || tactics !== null
  /** Tácticas a qualquer momento: parado abre já; a correr, espera que a bola saia. */
  const openTactics = (side: 0 | 1) => {
    if (view.finished || tactics !== null) return
    if (running) setWaiting((w) => (w?.side === side ? null : { side, since: view.minute, from: shown }))
    else {
      resumeAfter.current = false
      setTactics(side)
    }
  }
  const closeTactics = () => {
    setTactics(null)
    setInjuryNote(null)
    // Se o jogo corria quando as pediu, continua sozinho
    if (resumeAfter.current) setRunning(true)
    resumeAfter.current = false
  }
  const bottom: [Pane, string][] = [
    ['homeData', `Dados ${view.home.club.shortName}`],
    ['ratings', 'Classificações Jogadores'],
    ['results', 'Últimos Resultados'],
    ['table', 'Classificação Geral'],
    ['awayData', `Dados ${view.away.club.shortName}`],
  ]

  const period = view.finished
    ? 'Final'
    : view.halfTime
      ? 'Intervalo'
      : !started
        ? 'Antes do Jogo'
        : view.minute <= 45
          ? '1ª Parte'
          : '2ª Parte'

  // Por baixo da barra do topo (h-11), que continua à vista com o Continuar Jogo. Vai para o
  // palco por portal: cobre a coluna sem sair das barras pretas (o `fixed` media-se pela
  // janela). Escondida, o `hidden` do Shell não chega cá
  const stage = document.getElementById('stage')
  const screen = (
    <div className={`absolute inset-x-0 bottom-0 top-11 z-50 flex bg-[#121318] ${active ? '' : 'hidden'}`}>
      <aside className="flex w-[196px] shrink-0 flex-col gap-1.5 border-r border-cm-line bg-cm-deep p-2">
        <div className="flex flex-col items-center rounded bg-cm-raised py-2">
          <span className="text-[11px] font-bold font-mono uppercase tracking-widest text-cm-dim">Minuto de Jogo</span>
          <span className="font-title text-[44px] font-bold leading-[48px] text-cm-orange">{view.minute}'</span>
          <span className="text-[12px] font-bold font-mono uppercase tracking-wider text-cm-mint">{period}</span>
        </div>
        <div className="flex gap-1.5">
          <button className="flex h-8 flex-1 items-center justify-center rounded bg-cm-raised text-cm-faint" disabled aria-label="Voltar">
            ◀
          </button>
          <button className="flex h-8 flex-1 items-center justify-center rounded bg-cm-raised text-cm-faint" disabled aria-label="Avançar">
            ▶
          </button>
        </div>
        <MatchSideButton onClick={main} disabled={finishing || tactics !== null} className="font-mono uppercase tracking-wide">
          {running ? <Pause size={15} fill="currentColor" /> : <Play size={15} fill="currentColor" />}
          {label}
        </MatchSideButton>
        {([0, 1] as const).map((side) => {
          const club = (side === 0 ? view.home : view.away).club
          return (
            <MatchSideButton
              key={side}
              onClick={() => openTactics(side)}
              disabled={tactics !== null || view.finished}
              style={clubBand(club)}
              className={`!justify-between text-left ${waiting?.side === side ? 'ring-2 ring-inset ring-cm-yellow' : ''}`}
            >
              <span className="truncate">Tácticas {club.shortName}</span>
              <SlidersHorizontal size={15} className="shrink-0" />
            </MatchSideButton>
          )
        })}
        {view.userSide !== null && !running && (
          <MatchSideButton onClick={() => setTalk(true)} disabled={running || tactics !== null || talk}>
            <MessageSquare size={15} />
            Palestra
          </MatchSideButton>
        )}
        <div className="flex flex-col gap-1 rounded bg-cm-raised p-2">
          <span className="mb-0.5 text-[11px] font-bold font-mono uppercase tracking-widest text-cm-dim">Velocidade Comentário</span>
          {MATCH_SPEEDS.map((s) => (
            <button
              key={s.key}
              disabled={locked}
              onClick={() => saveSettings({ ...getSettings(), matchSpeed: s.key })}
              className={`h-6 rounded text-[12px] disabled:opacity-50 ${
                s.key === speed.key ? 'bg-cm-mint font-bold text-cm-onmint' : 'bg-cm-deep text-cm-muted hover:text-white'
              }`}
            >
              {s.label}
            </button>
          ))}
        </div>
      </aside>

      <main className="flex min-w-0 flex-1 flex-col gap-1.5 p-2">
        {talk ? (
          <TalkPanel onClose={() => setTalk(false)} />
        ) : tactics !== null ? (
          <LiveTactics view={view} side={tactics} notice={injuryNote} onClose={closeTactics} onApplied={accept} />
        ) : (
          <>
            <div className="flex shrink-0 gap-1.5">
              <ScoreBand
                club={view.home.club}
                goals={goalsSaid(0)}
                side="home"
                detail={['Casa', view.venue].filter(Boolean).join(' · ')}
                onClick={view.finished ? undefined : () => openTactics(0)}
              />
              <ScoreBand
                club={view.away.club}
                goals={goalsSaid(1)}
                side="away"
                detail="Fora"
                onClick={view.finished ? undefined : () => openTactics(1)}
              />
            </div>
            {waiting && (
              <div className="flex h-6 shrink-0 items-center justify-center rounded bg-black/70 text-[13px] font-bold text-cm-yellow">
                Tácticas {(waiting.side === 0 ? view.home : view.away).club.shortName}: à espera que a bola saia…
              </div>
            )}
            <div className="flex shrink-0 gap-1.5">
              {(
                [
                  ['panorama', 'Panorama Jogo'],
                  ['stats', 'Estatísticas Jogo'],
                  ['zones', 'Zonas Acção'],
                  ['report', 'Relatório Jogo'],
                  ['analysis', 'Análise'],
                  ...(view.finished ? [['post', 'Pós-Jogo']] : []),
                ] as [Pane, string][]
              ).map(([k, t]) => (
                <button key={k} className={matchTab(pane === k)} onClick={() => setPane(k)}>
                  {t}
                </button>
              ))}
            </div>
            <div className="min-h-0 flex-1 overflow-auto rounded bg-cm-surface">
              {pane === 'panorama' && <Panorama view={view} line={current} said={said} started={started} />}
              {pane === 'stats' && <Stats view={view} />}
              {pane === 'zones' && <Zones view={view} />}
              {pane === 'report' && <Report lines={said} />}
              {pane === 'analysis' && <AnalysisPane view={view} />}
              {pane === 'post' && <PostMatchPane view={view} />}
              {pane === 'homeData' && <TeamData team={view.home} />}
              {pane === 'awayData' && <TeamData team={view.away} />}
              {pane === 'ratings' && <Ratings view={view} motm={view.finished ? (manOfTheMatch(view, said)?.player.id ?? null) : null} />}
              {pane === 'results' && <Results view={view} own={own} />}
              {pane === 'table' && <Table view={view} own={own} />}
            </div>
            {error && <div className="text-cm-red">{error}</div>}
            <div className="flex shrink-0 gap-1.5">
              {bottom.map(([k, t]) => (
                <button key={k} className={`${matchTab(pane === k)} text-[13px]`} onClick={() => setPane(k)}>
                  {t}
                </button>
              ))}
            </div>
            <div className="flex h-10 shrink-0 items-center justify-center gap-2 rounded bg-cm-purple font-title text-[20px] font-bold uppercase text-white">
              <Landmark size={18} className="shrink-0" />
              {[view.venue, view.city].filter(Boolean).join(', ') || view.home.club.name}
            </div>
            <div className="flex h-[112px] shrink-0 gap-1.5">
              <Highlights team={view.home} />
              <History said={said} />
              <Highlights team={view.away} />
            </div>
          </>
        )}
      </main>
    </div>
  )
  return stage ? createPortal(screen, stage) : screen
}
