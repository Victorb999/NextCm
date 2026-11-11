import { Landmark, MessageSquare, Pause, Play, SlidersHorizontal, Trophy } from 'lucide-react'
import { useEffect, useImperativeHandle, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import { createPortal } from 'react-dom'
import { api } from '../api'
import type { ClubRef } from '../bindings/ClubRef'
import type { Fixture } from '../bindings/Fixture'
import type { LivePlayer } from '../bindings/LivePlayer'
import type { LiveTeam } from '../bindings/LiveTeam'
import type { LiveView } from '../bindings/LiveView'
import type { StandingRow } from '../bindings/StandingRow'
import { initials, longDate, shortListName } from '../format'
import { useGame } from '../game'
import { liveLineup } from '../matchLineup'
import { useNav } from '../nav'
import * as sound from '../sound'
import type { MatchPhase } from '../shell/useMatchDay'
import { MATCH_SPEEDS, getSettings, saveSettings, useSettings } from '../settings'
import { conditionColour, MENTALITY, PASSING, TACKLING } from '../tactics'
import LiveTactics from './LiveTactics'
import PitchField from './PitchField'
import PreMatch, { AnalysisPane, PostMatchPane } from './PreMatch'
import TalkPanel from './TalkPanel'
import MatchLineups from './MatchLineups'
import { BottomBar, Link, Panel, ScreenHeader } from './Screen'

const clubStyle = (c: ClubRef) => (c.fg && c.bg ? { color: c.fg, background: c.bg } : { color: '#fff', background: '#3a3a3a' })

/** As cores do clube com um brilho por cima (placar, botões das Tácticas). */
const clubBand = (c: ClubRef) => ({
  color: c.fg && c.bg ? c.fg : '#fff',
  backgroundColor: c.fg && c.bg ? c.bg : '#3a3a3a',
  backgroundImage: 'linear-gradient(105deg, rgba(255,255,255,0.16), transparent 45%, rgba(0,0,0,0.2))',
})

/** Faixa de um clube no resultado: emblema, nome grande nas cores do clube e o placar numa casa. */
function ScoreBand({
  club,
  goals,
  side,
  detail,
  onClick,
}: {
  club: ClubRef
  goals: number | null
  side: 'home' | 'away'
  detail: string
  onClick?: () => void
}) {
  return (
    <div
      className={`flex h-16 min-w-0 flex-1 items-center gap-3 rounded px-3 shadow-lg ${side === 'away' ? 'flex-row-reverse' : ''} ${onClick ? 'cursor-pointer' : ''}`}
      style={clubBand(club)}
      onClick={onClick}
      title={onClick ? `Tácticas ${club.shortName}` : undefined}
    >
      <span className="flex size-10 shrink-0 items-center justify-center rounded bg-black/25 font-title text-[17px] font-bold">
        {initials(club.shortName)}
      </span>
      <div className={`min-w-0 flex-1 ${side === 'away' ? 'text-right' : ''}`}>
        <div className="truncate font-title text-[28px] font-bold uppercase ">{club.shortName}</div>
        <div className="truncate text-[11px] font-bold font-mono uppercase tracking-wider opacity-80">{detail}</div>
      </div>
      {/* O número muda com o golo e dá um pulo (o 0 do início, não) */}
      <span
        key={goals ?? '-'}
        className={`flex h-12 min-w-[54px] shrink-0 items-center justify-center rounded bg-[#0d0e13]/90 px-2 font-title text-[34px] font-bold leading-none text-white shadow-[inset_0_2px_4px_rgba(0,0,0,0.8)] ${goals ? 'cm-score-pop' : ''}`}
      >
        {goals ?? ''}
      </span>
    </div>
  )
}

/** Faixa azul com o nome da competição, como nas telas de jogos do dia do CM. */
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
  const nav = useNav()
  // Os clubes são ligações; com resultado, o placar abre a ficha da partida (esconde o dia de jogo, como qualquer navegação)
  const name = (c: ClubRef, align: string) => (
    <span className={`flex-1 truncate text-[17px] font-semibold ${align} ${c.id === own ? 'text-cm-yellow' : 'text-white'}`}>
      <Link onClick={() => nav.go({ kind: 'club', id: c.id })}>{c.shortName}</Link>
    </span>
  )
  return (
    <div className="cm-backdrop absolute inset-0 z-50 flex flex-col">
      <ScreenHeader
        icon={
          <span className="flex size-11 items-center justify-center rounded bg-[#1e9cf0] text-white">
            <Trophy size={22} />
          </span>
        }
        eyebrow={fixture.competition}
        title={`${title} · ${fixture.label}`}
      />
      <Panel title={`${games.length} jogos`} className="mx-2 min-h-0 flex-1">
        <div className="cm-zebra min-h-0 flex-1 overflow-auto">
          {games.map((f) => (
            <div key={f.id} className="flex h-[32px] items-center gap-3 px-4">
              <span className="cm-cell w-11 shrink-0 text-center font-mono text-[12px] leading-[22px] text-white">{pos(f.home.id)}</span>
              {name(f.home, 'text-right')}
              {scores && f.homeGoals !== null ? (
                <button
                  onClick={() => nav.go({ kind: 'match', id: f.id })}
                  title="Ver a ficha da partida"
                  className="w-16 shrink-0 rounded bg-black/40 text-center font-mono text-[17px] font-bold leading-[26px] text-cm-magenta hover:bg-cm-sel hover:text-white"
                >
                  {f.homeGoals}:{f.awayGoals}
                </button>
              ) : (
                <span className="w-16 shrink-0 rounded bg-black/40 text-center font-mono text-[17px] font-bold leading-[26px] text-cm-magenta">
                  v
                </span>
              )}
              {name(f.away, 'text-left')}
              <span className="cm-cell w-11 shrink-0 text-center font-mono text-[12px] leading-[22px] text-white">{pos(f.away.id)}</span>
            </div>
          ))}
        </div>
      </Panel>
      <BottomBar onRight={onNext} />
    </div>
  )
}

// ---------------------------------------------------------------------------------------
// A partida

/** Uma frase do relato, com o lado que a faz (para as cores da caixa). */
type Line = {
  minute: number
  side: number | null
  text: string
  kind: string
  playerId: number | null
  last: boolean
}

/** A bola parou (saiu, falta, golo, defesa, substituição...): é aí que se pode ir às Tácticas. */
const isStoppage = (l: Line) =>
  (l.last && ['goal', 'save', 'miss', 'yellow', 'red', 'injury', 'sub', 'tactics', 'half', 'end'].includes(l.kind)) ||
  (l.kind === 'play' && /lançamento|repôs|agarra/.test(l.text))

/** A frase do golo em si ("GOLO! ..."), a última de cada lance de golo. */
const isGoalLine = (l: Line) => l.kind === 'goal' && l.text.startsWith('GOLO')

/** Barra de posse no caixilho roxo, com as cores dos clubes e os nomes por cima. */
function PossessionBar({ view, home, label }: { view: LiveView; home: number; label: string }) {
  const seg = (c: ClubRef, pct: number, align: string) => (
    <div className={`flex items-center px-2 text-[11px] font-bold ${align}`} style={{ ...clubStyle(c), width: `${pct}%` }}>
      {pct > 8 ? `${pct}%` : ''}
    </div>
  )
  return (
    <div className="flex flex-col gap-1.5 rounded border border-cm-purple/60 bg-cm-purple/25 px-3 py-2">
      <div className="flex items-center justify-between text-[12px] font-bold font-mono uppercase tracking-wider">
        <span className="text-cm-lilac">{label}</span>
        <span className="normal-case tracking-normal text-cm-text">
          {view.home.club.shortName} {home}% <span className="text-cm-dim">vs</span> {view.away.club.shortName} {100 - home}%
        </span>
      </div>
      <div className="flex h-4 overflow-hidden rounded bg-black">
        {seg(view.home.club, home, 'justify-end')}
        <div className="w-1 shrink-0 bg-cm-orange" />
        {seg(view.away.club, 100 - home, 'justify-start')}
      </div>
    </div>
  )
}

/** O nome do lance na caixa do relato; os de perigo só dizem o que foi na última frase. */
function lineLabel(l: Line): string {
  if (l.kind === 'goal') return isGoalLine(l) ? 'Golo' : 'Lance Perigoso'
  if (l.kind === 'save' || l.kind === 'miss') return l.last ? (l.kind === 'save' ? 'Defesa' : 'Remate Falhado') : 'Lance Perigoso'
  const labels: Record<string, string> = {
    yellow: 'Cartão Amarelo',
    red: 'Cartão Vermelho',
    injury: 'Lesão',
    sub: 'Substituição',
    tactics: 'Tácticas',
    half: 'Intervalo',
    end: 'Final',
    start: 'Pontapé de Saída',
  }
  return labels[l.kind] ?? 'Lance'
}

/** A frase do relato em destaque: cartão branco com a cor de quem a faz; o golo nas cores do clube. */
function LineCard({ view, line }: { view: LiveView; line: Line }) {
  const team = line.side === 0 ? view.home : line.side === 1 ? view.away : null
  const goal = isGoalLine(line) && team
  const stripe = team?.club.bg ?? `linear-gradient(90deg, ${view.home.club.bg ?? '#888'}, #888, ${view.away.club.bg ?? '#888'})`
  return (
    <div
      className={`relative w-full max-w-[760px] overflow-hidden rounded-lg px-5 pb-3.5 pt-4
        shadow-[0_12px_32px_rgba(0,0,0,0.8)] ${goal ? 'cm-goal-in' : 'cm-line-in bg-white text-[#141414]'}`}
      style={team ? clubBand(team.club) : { background: stripe }}
    >
      {!goal && <div className="absolute inset-x-0 top-0 h-1" style={{ background: stripe }} />}
      <div className="flex items-center gap-4">
        {team && (
          <span
            className="flex size-10 shrink-0 items-center justify-center rounded-full border-2 border-black/20 font-title text-[14px] font-bold"
            style={clubStyle(team.club)}
          >
            {initials(team.club.shortName)}
          </span>
        )}
        <div className="min-w-0 flex-1">
          <div className="text-[11px] font-bold font-mono uppercase tracking-widest opacity-60">
            {lineLabel(line)} · {line.minute}'
          </div>
          <div className={`font-title text-[24px] leading-tight ${goal ? 'font-bold' : ''}`}>{line.text}</div>
        </div>
      </div>
    </div>
  )
}

/** As linhas do campo, apagadas, por trás do Panorama. */
function PitchLines() {
  return (
    <div className="pointer-events-none absolute inset-x-6 bottom-28 top-10 border border-white/[0.07]">
      <div className="absolute inset-y-0 left-1/2 border-l border-white/[0.07]" />
      <div className="absolute left-1/2 top-1/2 size-28 -translate-x-1/2 -translate-y-1/2 rounded-full border border-white/[0.07]" />
      <div className="absolute left-0 top-1/4 h-1/2 w-[13%] border border-l-0 border-white/[0.07]" />
      <div className="absolute right-0 top-1/4 h-1/2 w-[13%] border border-r-0 border-white/[0.07]" />
    </div>
  )
}

/** Marcadores de um lado (apelido e minutos), só dos golos que o relato já contou. */
function Scorers({ view, said, side, small }: { view: LiveView; said: Line[]; side: 0 | 1; small?: boolean }) {
  const team = side === 0 ? view.home : view.away
  const goals = new Map<string, number[]>()
  // As lesões também, a verde ("Mendoza lesionado 45"), como no CM
  const injuries: [string, number][] = []
  for (const l of said) {
    if (l.side !== side) continue
    const p = [...team.players, ...team.out].find((p) => p.id === l.playerId)
    const name = p ? p.listName.split(', ')[0] : '?'
    if (isGoalLine(l)) goals.set(name, [...(goals.get(name) ?? []), l.minute])
    else if (l.kind === 'injury' && l.last) injuries.push([name, l.minute])
  }
  return (
    <div className={`flex flex-1 flex-col ${small ? 'gap-0' : 'gap-1'} ${side === 1 ? 'items-start pl-8' : 'items-end pr-8'}`}>
      {[...goals].map(([name, mins]) => (
        <div key={name} className={`flex font-title font-bold ${small ? 'gap-4 text-[17px] leading-[22px]' : 'gap-8 text-[22px]'}`}>
          <span className="text-cm-text">{name}</span>
          <span className="text-cm-yellow">{mins.join(', ')}</span>
        </div>
      ))}
      {injuries.map(([name, minute]) => (
        <div
          key={`${name}-${minute}`}
          className={`flex font-title font-bold ${small ? 'gap-4 text-[17px] leading-[22px]' : 'gap-8 text-[22px]'}`}
        >
          <span className="text-[#4ade80]">{name} lesionado</span>
          <span className="text-cm-yellow">{minute}</span>
        </div>
      ))}
    </div>
  )
}

/** O melhor em campo (o último lance do relato), quando o relato já o disse. */
function manOfTheMatch(view: LiveView, said: Line[]) {
  const l = said.find((l) => l.kind === 'end' && l.playerId !== null)
  if (!l || l.side === null) return null
  const team = l.side === 0 ? view.home : view.away
  const p = [...team.players, ...team.out].find((p) => p.id === l.playerId)
  return p ? { player: p, club: team.club } : null
}

function Panorama({ view, line, said, started }: { view: LiveView; line: Line | null; said: Line[]; started: boolean }) {
  // Golos ou lesões: o bloco dos acontecimentos em cima
  const anyGoal = said.some((l) => isGoalLine(l) || (l.kind === 'injury' && l.last))
  const motm = view.finished ? manOfTheMatch(view, said) : null
  const title = !started
    ? 'Pontapé Saída'
    : view.finished
      ? 'Final'
      : view.halfTime
        ? 'Intervalo'
        : view.minute < 45
          ? 'Primeira Parte'
          : 'Segunda Parte'
  const score = view.halfTime
    ? ` · INT ${view.homeGoals}-${view.awayGoals}`
    : view.finished
      ? ` · FIN ${view.homeGoals}-${view.awayGoals}`
      : ''
  // Os cartões que o relato já mostrou
  const cards = (kind: string) => said.filter((l) => l.kind === kind && l.last).length
  const [yellows, reds] = [cards('yellow'), cards('red')]
  return (
    <div className="relative flex h-full flex-col">
      <PitchLines />
      <div className="relative flex h-10 shrink-0 items-center justify-between px-3 text-[13px]">
        <span>
          <span className="font-bold font-mono uppercase tracking-wider text-cm-orange">{title}</span>
          <span className="text-cm-dim"> / </span>
          <span className="text-cm-muted">
            {longDate(view.fixture.date)} {view.fixture.date.slice(0, 4)}
          </span>
        </span>
        <span className="rounded bg-black/50 px-2 py-0.5 text-[12px] font-bold font-mono uppercase tracking-wider text-cm-mint">
          {view.fixture.competition}
          {score}
        </span>
      </div>
      {/* Durante o jogo, os marcadores ficam em cima, longe da caixa do relato */}
      {!(view.halfTime || view.finished) && anyGoal && (
        <div className="relative flex shrink-0 py-1.5">
          <Scorers view={view} said={said} side={0} small />
          <Scorers view={view} said={said} side={1} small />
        </div>
      )}
      <div className="relative flex min-h-0 flex-1 items-center justify-center px-8">
        {view.halfTime || view.finished ? (
          <div className="flex w-full flex-col items-center gap-5">
            <div className="flex w-full">
              <Scorers view={view} said={said} side={0} />
              <Scorers view={view} said={said} side={1} />
            </div>
            {motm && (
              <div className="flex min-w-[360px] flex-col items-center rounded-lg border-2 border-cm-yellow bg-black/70 px-8 py-2">
                <span className="text-[13px] font-bold font-mono uppercase tracking-wider text-cm-yellow">Melhor em Campo</span>
                <span className="font-title text-[28px] font-bold leading-tight text-white">
                  {motm.player.listName.split(', ').reverse().join(' ')}
                </span>
                <span className="flex items-center gap-3 text-[14px]">
                  <span className="rounded px-2 font-bold" style={clubStyle(motm.club)}>
                    {motm.club.shortName}
                  </span>
                  <span className="font-bold text-cm-magenta">Nota {motm.player.rating ?? '-'}</span>
                </span>
              </div>
            )}
          </div>
        ) : (
          // Uma frase nova é um cartão novo (entra com a animação)
          line && <LineCard key={said.length} view={view} line={line} />
        )}
      </div>
      <div className="relative mx-3 mb-2 shrink-0">
        <PossessionBar view={view} home={view.recentPossession} label="Posse · últimos 5 min" />
      </div>
      <div className="relative flex h-8 shrink-0 items-center gap-3 px-3 text-[13px]">
        <span>
          <span className="font-bold text-cm-orange">Árbitro:</span> <span className="text-white">{view.referee}</span>
        </span>
        <span className="text-cm-faint">•</span>
        <span className="flex items-center gap-1.5 text-cm-muted">
          Cartões:
          {yellows + reds === 0 && <span className="text-cm-dim">-</span>}
          {yellows > 0 && <span className="rounded bg-cm-yellow/20 px-1.5 text-[11px] font-bold text-cm-yellow">{yellows} Amr</span>}
          {reds > 0 && <span className="rounded bg-cm-red/20 px-1.5 text-[11px] font-bold text-cm-red">{reds} Verm</span>}
        </span>
        <span className="ml-auto text-cm-orange">
          Tempo: <span className="text-white">{view.weather}</span>
        </span>
        {started && (
          <>
            <span className="text-cm-faint">•</span>
            <span className="text-cm-muted">
              Assistência: <span className="text-white">{view.attendance.toLocaleString('pt-PT')}</span>
            </span>
          </>
        )}
      </div>
    </div>
  )
}

const pct = (ok: number, all: number) => (all > 0 ? `${Math.round((100 * ok) / all)}%` : '-')

function Stats({ view }: { view: LiveView }) {
  const [h, a] = [view.home.stats, view.away.stats]
  const rows: [string, string | number, string | number, string?][] = [
    ['Remates à Baliza', h.shots, a.shots],
    ['No Alvo', h.onTarget, a.onTarget],
    ['Ao Lado', h.shots - h.onTarget, a.shots - a.onTarget],
    ['Cantos', h.corners, a.corners],
    ['Livres', a.fouls, h.fouls],
    ['Lançamentos', h.throwIns, a.throwIns],
    ['Faltas', h.fouls, a.fouls],
    ['Foras de Jogo', h.offsides, a.offsides],
    ['Passes Acertados', pct(h.passesOk, h.passes), pct(a.passesOk, a.passes)],
    ['Cortes Ganhos', pct(h.tacklesWon, h.tackles), pct(a.tacklesWon, a.tackles)],
    ['Cabeceamentos Ganhos', pct(h.headersWon, h.headers), pct(a.headersWon, a.headers)],
    ['Cartões Amarelos', h.yellows, a.yellows, 'text-cm-yellow'],
    ['Cartões Vermelhos', h.reds, a.reds, 'text-cm-red'],
  ]
  return (
    <div className="px-2 py-1.5">
      {rows.map(([label, x, y, colour]) => (
        <div key={label} className="flex h-6 items-center gap-1">
          <span className="flex h-[25px] w-24 shrink-0 items-center justify-center bg-[#1a1a1a] font-title text-[18px] text-cm-magenta">
            {x}
          </span>
          <span className={`flex-1 text-center font-title text-[20px] ${colour ?? 'text-cm-text'}`}>{label}</span>
          <span className="flex h-[25px] w-24 shrink-0 items-center justify-center bg-[#1a1a1a] font-title text-[18px] text-cm-magenta">
            {y}
          </span>
        </div>
      ))}
    </div>
  )
}

function Zones({ view }: { view: LiveView }) {
  const t = view.usefulTime
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 p-3">
      <div
        className="relative flex h-[60%] min-h-[200px] w-[78%] border-2 border-[#d8d8d8]/60"
        style={{
          background: 'repeating-linear-gradient(90deg, rgb(var(--c-pitch)) 0 44px, #126a1a 44px 88px)',
        }}
      >
        <div className="absolute left-0 top-1/4 h-1/2 w-[12%] border border-l-0 border-white/50" />
        <div className="absolute right-0 top-1/4 h-1/2 w-[12%] border border-r-0 border-white/50" />
        <div className="absolute left-1/2 top-1/2 h-20 w-20 -translate-x-1/2 -translate-y-1/2 rounded-full border border-white/50" />
        <div className="absolute left-1/2 top-0 h-full border-l border-white/40" />
        <div className="absolute left-1/2 top-2 -translate-x-1/2 bg-black/60 px-12 text-white">Global</div>
        <div className="absolute right-3 top-2 text-[13px] text-cm-yellow">
          Tempo útil : {Math.floor(t / 60)}' {t % 60}
        </div>
        {[0, 1, 2].map((i) => (
          <div
            key={i}
            className={`relative flex flex-1 flex-col items-center justify-center ${i < 2 ? 'border-r border-dashed border-white/80' : ''}`}
          >
            <span className="font-title text-[26px] font-bold text-cm-yellow">{view.zones[i]}%</span>
            <div className="absolute bottom-2 left-2 right-2 flex h-4 border-2 border-[#3a1470] bg-white">
              <div
                style={{
                  width: `${view.zonePossession[i]}%`,
                  background: view.home.club.bg ?? '#2020c0',
                }}
              />
            </div>
          </div>
        ))}
      </div>
      <div className="w-[78%]">
        <PossessionBar view={view} home={view.home.stats.possession} label="Posse de bola" />
      </div>
    </div>
  )
}

function Report({ lines }: { lines: Line[] }) {
  const box = useRef<HTMLDivElement>(null)
  // A frase mais recente fica à vista (só o painel rola)
  useEffect(() => {
    const panel = box.current?.parentElement
    if (panel) panel.scrollTop = panel.scrollHeight
  }, [lines.length])
  return (
    <div ref={box} className="px-2 py-1">
      {lines.map((l, i) => (
        <div key={i} className="flex h-[19px] items-center border-b border-white/5 text-[13px]">
          <span className="w-24 shrink-0 pl-6 font-bold text-cm-yellow">
            {i === 0 || lines[i - 1].minute !== l.minute ? `${l.minute} min` : ''}
          </span>
          <span
            className={`truncate ${l.kind === 'goal' ? 'font-bold text-cm-yellow' : l.kind === 'red' ? 'text-cm-red' : 'text-cm-text'}`}
          >
            {l.text}
          </span>
        </div>
      ))}
    </div>
  )
}

const fullName = (p: LivePlayer) => p.listName.split(', ').reverse().join(' ')

/** A cor de uma nota (1–10): verde das boas, amarela das médias, vermelha das más. */
const ratingColour = (r: number | null) =>
  r === null ? '#5a5a6a' : r >= 8 ? 'rgb(var(--c-mint))' : r >= 7 ? '#b6e35a' : r >= 6 ? '#f2d24b' : r >= 5 ? '#f29a3b' : '#ff5a5a'

/** O campo dos Dados da equipa, deitado (o ataque para a direita): quem está em campo no seu
 * lugar, com o número e a nota na cor. */
function TeamPitch({ team }: { team: LiveTeam }) {
  const on = team.players.filter((p) => p.x !== null && p.y !== null && !p.red)
  return (
    <div className="flex min-w-0 flex-[3] flex-col">
      <div className="h-[18px] bg-cm-head px-2 text-[12px] font-bold uppercase leading-[18px] tracking-wide text-white">
        Em Campo · {team.formation}
      </div>
      <PitchField landscape className="mt-1 w-full">
        {on.map((p) => (
          <div
            key={p.id}
            className="absolute flex -translate-x-1/2 -translate-y-1/2 flex-col items-center"
            // De pé o ataque é para cima (y pequeno); deitado, para a direita
            style={{ left: `${100 - (p.y ?? 0)}%`, top: `${p.x}%` }}
            title={`${fullName(p)} · ${p.role ?? ''}`}
          >
            <span
              className="flex size-7 items-center justify-center rounded-md text-[13px] font-bold text-[#141414] shadow-[0_2px_6px_rgba(0,0,0,0.5)]"
              style={{ background: ratingColour(p.rating) }}
            >
              {p.squadNumber ?? ''}
            </span>
            <span className="mt-0.5 whitespace-nowrap rounded bg-black/75 px-1 text-[11px] font-bold leading-[15px] text-white">
              {shortListName(p.listName)}
              {p.rating !== null && <span style={{ color: ratingColour(p.rating) }}> {p.rating}</span>}
              {p.goals > 0 && <span className="text-cm-yellow"> {'●'.repeat(p.goals)}</span>}
              {p.yellow && <span className="text-cm-yellow"> ▮</span>}
            </span>
          </div>
        ))}
      </PitchField>
    </div>
  )
}

/** As notas de quem jogou em barras, da melhor para a pior (com os golos e os cartões). */
function RatingBars({ players }: { players: LivePlayer[] }) {
  const rated = players.filter((p) => p.rating !== null).sort((a, b) => (b.rating ?? 0) - (a.rating ?? 0))
  return (
    <div className="flex min-w-[220px] flex-[2] flex-col">
      <div className="h-[18px] bg-cm-head px-2 text-[12px] font-bold uppercase leading-[18px] tracking-wide text-white">Notas</div>
      <div className="mt-1 flex flex-col gap-[3px]">
        {rated.map((p) => (
          <div key={p.id} className="flex h-[19px] items-center gap-2 text-[13px]">
            <span className="w-32 shrink-0 truncate text-white">{shortListName(p.listName)}</span>
            <div className="relative h-[13px] min-w-0 flex-1 rounded-sm bg-black/40">
              <div
                className="h-full rounded-sm"
                style={{ width: `${((p.rating ?? 0) / 10) * 100}%`, background: ratingColour(p.rating) }}
              />
            </div>
            <span className="w-5 shrink-0 text-right font-bold" style={{ color: ratingColour(p.rating) }}>
              {p.rating}
            </span>
            <span className="w-10 shrink-0 truncate text-cm-yellow">
              {p.goals > 0 ? '●'.repeat(p.goals) : ''}
              {p.red ? <span className="text-cm-red"> ▮</span> : p.yellow ? ' ▮' : ''}
            </span>
          </div>
        ))}
      </div>
    </div>
  )
}

/** Uma caixa dos Dados da equipa: o título e as linhas "rótulo ... valor". */
function DataBox({ title, rows }: { title: string; rows: [string, ReactNode][] }) {
  return (
    <div className="min-w-0 flex-1 rounded bg-black/25">
      <div className="h-[20px] bg-cm-head px-2 text-[12px] font-bold uppercase leading-[20px] tracking-wide text-white">{title}</div>
      <div className="cm-zebra">
        {rows.map(([label, value]) => (
          <div key={label} className="flex h-[19px] items-center gap-2 px-2 text-[13px]">
            <span className="truncate text-[#c9c9d6]">{label}</span>
            <span className="ml-auto shrink-0 truncate text-cm-yellow">{value}</span>
          </div>
        ))}
      </div>
    </div>
  )
}

/** "Dados <clube>": os números de cada jogador, os totais, os da equipa, os destaques e a táctica. */
function TeamData({ team }: { team: LiveTeam }) {
  const cols = ['Pas', 'Cmp', 'Crt', 'Gnh', 'Cab', 'Gnh', 'Dri', 'Ast', 'Rem', 'Bal', 'Con', 'Nt', 'G']
  const played = [...team.players, ...team.out]
  const sum = (f: (p: LivePlayer) => number) => played.reduce((n, p) => n + f(p), 0)
  // O melhor de cada coisa entre quem jogou ("-" se ninguém tem nada)
  const best = (f: (p: LivePlayer) => number, show: (p: LivePlayer) => string = (p) => String(f(p))) => {
    const top = played.reduce<LivePlayer | null>((b, p) => (f(p) > 0 && (!b || f(p) > f(b)) ? p : b), null)
    return top ? `${shortListName(top.listName)} (${show(top)})` : '-'
  }
  const s = team.stats
  const onPitch = team.players.filter((p) => !p.red)
  const condition = onPitch.length ? Math.round(onPitch.reduce((n, p) => n + p.condition, 0) / onPitch.length) : 0
  const scorers = played.filter((p) => p.goals > 0)
  const i = team.instructions
  const yes = (v: boolean) => (v ? 'Sim' : 'Não')
  const row = (p: LivePlayer, played: boolean) => {
    const s = p.stats
    const vals = played
      ? [s.passes, s.passesOk, s.tackles, s.tacklesWon, s.headers, s.headersWon, s.dribbles, p.assists, s.shots, s.onTarget]
      : []
    return (
      <div key={p.id} className="flex h-[19px] items-center gap-1 text-[13px]">
        <span
          className={`h-full w-7 shrink-0 text-center leading-[19px] ${played ? 'bg-[#262626] text-white' : 'bg-[#161660] text-cm-lilac'}`}
        >
          {p.squadNumber ?? ''}
        </span>
        <span className="flex w-3 shrink-0 justify-center">
          {p.red ? <span className="h-3 w-2 bg-cm-red" /> : p.yellow ? <span className="h-3 w-2 bg-cm-yellow" /> : null}
        </span>
        <span
          className={`w-48 shrink-0 truncate ${!played ? 'text-cm-purplelight' : p.offMinute !== null && !p.red ? 'text-cm-lilac' : 'text-white'}`}
        >
          {fullName(p)}
          {p.offMinute !== null && !p.red ? <span className="text-cm-red"> ↓{p.offMinute}'</span> : null}
          {p.onMinute !== null ? <span className="text-cm-green"> ↑{p.onMinute}'</span> : null}
        </span>
        {vals.map((v, i) => (
          <span key={i} className={`w-9 shrink-0 text-center ${i < 2 ? 'text-cm-orange' : 'text-cm-yellow'}`}>
            {v}
          </span>
        ))}
        {played && (
          <>
            <span className="w-9 shrink-0 text-center text-[#40a0ff]">{p.condition}</span>
            <span className="w-9 shrink-0 text-center text-cm-magenta">{p.rating ?? ''}</span>
            <span className="w-9 shrink-0 text-center text-white">{p.goals || ''}</span>
          </>
        )}
      </div>
    )
  }
  return (
    <div className="px-3 py-2">
      <div className="flex gap-3">
        <div className="shrink-0">
          <div className="mb-1 flex h-[18px] items-center gap-1 text-[12px]">
            <span className="w-7 shrink-0" />
            <span className="w-3 shrink-0" />
            <span className="w-48 shrink-0" />
            {cols.map((c, i) => (
              <span key={i} className="w-9 shrink-0 bg-cm-head text-center font-bold text-white">
                {c}
              </span>
            ))}
          </div>
          <div className="cm-zebra">
            {played.map((p) => row(p, true))}
            {team.bench.map((p) => row(p, false))}
          </div>
          {/* Os totais de quem jogou, por baixo das colunas */}
          <div className="flex h-[20px] items-center gap-1 border-t border-white/20 text-[13px] font-bold">
            <span className="w-7 shrink-0" />
            <span className="w-3 shrink-0" />
            <span className="w-48 shrink-0 text-white">Totais</span>
            {[
              s.passes,
              s.passesOk,
              s.tackles,
              s.tacklesWon,
              s.headers,
              s.headersWon,
              sum((p) => p.stats.dribbles),
              sum((p) => p.assists),
              s.shots,
              s.onTarget,
            ].map((v, k) => (
              <span key={k} className={`w-9 shrink-0 text-center ${k < 2 ? 'text-cm-orange' : 'text-cm-yellow'}`}>
                {v}
              </span>
            ))}
            <span className="w-9 shrink-0 text-center text-[#40a0ff]">{condition || ''}</span>
            <span className="w-9 shrink-0" />
            <span className="w-9 shrink-0 text-center text-white">{sum((p) => p.goals) || ''}</span>
          </div>
        </div>
        {/* Ao lado da tabela: o campo com quem está em campo e as notas */}
        <TeamPitch team={team} />
        <RatingBars players={played} />
      </div>
      <div className="mt-2 flex gap-2">
        <DataBox
          title="Equipa"
          rows={[
            ['Posse de Bola', `${s.possession}%`],
            ['Remates (à baliza)', `${s.shots} (${s.onTarget})`],
            ['Passes Acertados', pct(s.passesOk, s.passes)],
            ['Cortes Ganhos', pct(s.tacklesWon, s.tackles)],
            ['Cabeceamentos Ganhos', pct(s.headersWon, s.headers)],
            ['Dribles', sum((p) => p.stats.dribbles)],
            ['Cantos', s.corners],
            ['Faltas', s.fouls],
            ['Foras de Jogo', s.offsides],
            ['Cartões', `${s.yellows} ${s.yellows === 1 ? 'amarelo' : 'amarelos'} · ${s.reds} ${s.reds === 1 ? 'vermelho' : 'vermelhos'}`],
          ]}
        />
        <DataBox
          title="Destaques"
          rows={[
            ['Melhor Nota', best((p) => p.rating ?? 0)],
            [
              'Golos',
              scorers.length ? scorers.map((p) => `${shortListName(p.listName)}${p.goals > 1 ? ` ${p.goals}` : ''}`).join(', ') : '-',
            ],
            ['Assistências', best((p) => p.assists)],
            ['Mais Remates', best((p) => p.stats.shots)],
            ['Mais Passes', best((p) => p.stats.passes)],
            [
              'Melhor Passe',
              best(
                (p) => (p.stats.passes >= 10 ? p.stats.passesOk / p.stats.passes : 0),
                (p) => pct(p.stats.passesOk, p.stats.passes),
              ),
            ],
            ['Mais Cortes Ganhos', best((p) => p.stats.tacklesWon)],
            ['Mais Dribles', best((p) => p.stats.dribbles)],
            ['Jogo Aéreo', best((p) => p.stats.headersWon)],
            [
              'Menos Condição',
              onPitch.length
                ? best(
                    (p) => 100 - p.condition,
                    (p) => `${p.condition}%`,
                  )
                : '-',
            ],
          ]}
        />
        <DataBox
          title="Táctica"
          rows={[
            ['Formação', team.formation],
            ['Mentalidade', MENTALITY.find(([k]) => k === i.mentality)?.[1] ?? '-'],
            ['Passe', PASSING.find(([k]) => k === i.passing)?.[1] ?? '-'],
            ['Desarme', TACKLING.find(([k]) => k === i.tackling)?.[1] ?? '-'],
            ['Pressão', yes(i.pressing)],
            ['Defesa em Linha', yes(i.offsideTrap)],
            ['Contra-Ataque', yes(i.counterAttack)],
            ['Defesa Intensa', yes(i.tightMarking)],
            ['Substituições', `${team.subsLeft} por fazer`],
            ['Condição Média', condition ? `${condition}%` : '-'],
          ]}
        />
      </div>
    </div>
  )
}

/** "Classificações Jogadores": as duas listas como no CM (quem saiu fica no lugar, quem entrou em baixo). */
function Ratings({ view, motm }: { view: LiveView; motm: number | null }) {
  return (
    <MatchLineups
      home={{ club: view.home.club, entries: liveLineup(view.home) }}
      away={{ club: view.away.club, entries: liveLineup(view.away) }}
      motm={motm}
    />
  )
}

function Results({ view, own }: { view: LiveView; own: number }) {
  const name = (c: ClubRef) => (
    <span className={`flex-1 truncate font-title text-[20px] ${c.id === own ? 'text-cm-yellow' : 'text-white'}`}>{c.shortName}</span>
  )
  if (view.others.length === 0) return <div className="p-4 text-[#aab3c4]">Não há outros jogos hoje.</div>
  return (
    <div className="px-6 pt-3 cm-zebra">
      <div className="mb-2 text-center text-[13px] text-cm-pink">
        {view.fixture.competition} · {view.minute}'
      </div>
      {view.others.map((o) => (
        <div key={o.fixture.id} className="flex h-[30px] items-center gap-3">
          {name(o.fixture.home)}
          <span className="w-16 shrink-0 text-center font-title text-[20px] font-bold text-cm-magenta">
            {o.homeGoals}:{o.awayGoals}
          </span>
          {name(o.fixture.away)}
        </div>
      ))}
    </div>
  )
}

function Table({ view, own }: { view: LiveView; own: number }) {
  const playing = [view.home.club.id, view.away.club.id]
  const heads = ['J', 'V', 'E', 'D', 'M', 'S', 'P']
  return (
    <div className="px-3 py-2 text-[13px]">
      <div className="flex h-5 items-center bg-cm-head font-bold text-cm-yellow">
        <span className="w-10 text-center" />
        <span className="flex-1">Clube</span>
        {heads.map((h) => (
          <span key={h} className="w-10 text-center">
            {h}
          </span>
        ))}
      </div>
      {view.table.map((r) => (
        <div
          key={r.club.id}
          className={`flex h-[19px] items-center border-b border-cm-line ${playing.includes(r.club.id) ? 'bg-white/10' : ''}`}
        >
          <span className="w-10 text-center text-white">{r.position}º</span>
          <span className={`flex-1 truncate ${r.club.id === own ? 'text-cm-pink' : 'text-white'}`}>{r.club.shortName}</span>
          {[r.played, r.won, r.drawn, r.lost, r.goalsFor, r.goalsAgainst, r.points].map((v, i) => (
            <span key={i} className={`w-10 text-center ${i === 6 ? 'font-bold text-cm-yellow' : 'text-cm-text'}`}>
              {v}
            </span>
          ))}
        </div>
      ))}
    </div>
  )
}

/** "<clube> · Destaques": os três com melhor nota em campo, com a condição. */
function Highlights({ team }: { team: LiveTeam }) {
  const top = [...team.players].sort((a, b) => (b.rating ?? 0) - (a.rating ?? 0)).slice(0, 3)
  return (
    <div className="min-w-0 flex-1 rounded bg-cm-surface px-2.5 py-2">
      <div className="mb-1 flex items-center justify-between text-[12px] font-bold font-mono uppercase tracking-wider">
        <span className="truncate text-cm-orange">{team.club.shortName} · Destaques</span>
        <span className="shrink-0 text-cm-dim">Con / Nota</span>
      </div>
      {top.map((p) => (
        <div key={p.id} className="flex h-6 items-center gap-2 text-[13px]">
          <span className="min-w-0 flex-1 truncate text-white">
            {shortListName(p.listName)} <span className="text-cm-dim">({p.role ?? p.position})</span>
          </span>
          <span className={`shrink-0 font-bold ${conditionColour(p.condition)}`}>{p.condition}%</span>
          <span className="w-6 shrink-0 rounded bg-[#34343a] text-center font-bold text-white">{p.rating ?? '-'}</span>
        </div>
      ))}
    </div>
  )
}

/** Os últimos lances que contam (golos, defesas, cartões, substituições), o mais recente em cima. */
function History({ said }: { said: Line[] }) {
  const kinds = ['save', 'miss', 'yellow', 'red', 'injury', 'sub']
  const recent = said
    .filter((l) => isGoalLine(l) || (l.last && kinds.includes(l.kind)))
    .slice(-3)
    .reverse()
  return (
    <div className="min-w-0 flex-[1.3] rounded bg-cm-surface px-2.5 py-2">
      <div className="mb-1 text-[12px] font-bold font-mono uppercase tracking-wider text-cm-orange">Histórico Recente</div>
      {recent.length === 0 && <div className="text-[13px] text-cm-dim">Ainda sem lances.</div>}
      {recent.map((l, i) => (
        <div key={i} className="flex h-6 items-center gap-2 text-[13px]">
          <span className="w-8 shrink-0 font-bold text-cm-orange">{l.minute}'</span>
          <span className={`truncate ${isGoalLine(l) ? 'font-bold text-cm-yellow' : l.kind === 'red' ? 'text-cm-red' : 'text-cm-text'}`}>
            {l.text}
          </span>
        </div>
      ))}
    </div>
  )
}

/** Aba da partida: escura e arredondada; a selecionada com borda e texto amarelos, como no CM. */
const matchTab = (selected: boolean) =>
  `flex h-8 min-w-0 flex-1 items-center justify-center truncate rounded px-2 ${
    selected
      ? 'bg-cm-hover font-bold text-cm-yellow ring-1 ring-inset ring-cm-yellow'
      : 'bg-cm-raised text-cm-muted hover:bg-cm-hover hover:text-white'
  }`

type Pane = 'panorama' | 'stats' | 'zones' | 'report' | 'homeData' | 'ratings' | 'results' | 'table' | 'awayData' | 'analysis' | 'post'

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
function LiveScreen({
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
  const speed = MATCH_SPEEDS.find((s) => s.key === settings.matchSpeed) ?? MATCH_SPEEDS[2]

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
  const current = shown > 0 ? lines[Math.min(shown, lines.length) - 1] : null
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
