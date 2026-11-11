import type { ClubRef } from '../../bindings/ClubRef'
import type { LivePlayer } from '../../bindings/LivePlayer'
import type { LiveView } from '../../bindings/LiveView'

export const clubStyle = (c: ClubRef) => (c.fg && c.bg ? { color: c.fg, background: c.bg } : { color: '#fff', background: '#3a3a3a' })

/** As cores do clube com um brilho por cima (placar, botões das Tácticas). */
export const clubBand = (c: ClubRef) => ({
  color: c.fg && c.bg ? c.fg : '#fff',
  backgroundColor: c.fg && c.bg ? c.bg : '#3a3a3a',
  backgroundImage: 'linear-gradient(105deg, rgba(255,255,255,0.16), transparent 45%, rgba(0,0,0,0.2))',
})

/** Uma frase do relato, com o lado que a faz (para as cores da caixa). */
export type Line = {
  minute: number
  side: number | null
  text: string
  kind: string
  playerId: number | null
  last: boolean
}

/** A bola parou (saiu, falta, golo, defesa, substituição...): é aí que se pode ir às Tácticas. */
export const isStoppage = (l: Line) =>
  (l.last && ['goal', 'save', 'miss', 'yellow', 'red', 'injury', 'sub', 'tactics', 'half', 'end'].includes(l.kind)) ||
  (l.kind === 'play' && /lançamento|repôs|agarra/.test(l.text))

/** A frase do golo em si ("GOLO! ..."), a última de cada lance de golo. */
export const isGoalLine = (l: Line) => l.kind === 'goal' && l.text.startsWith('GOLO')

/** O nome do lance na caixa do relato; os de perigo só dizem o que foi na última frase. */
export function lineLabel(l: Line): string {
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

/** O melhor em campo (o último lance do relato), quando o relato já o disse. */
export function manOfTheMatch(view: LiveView, said: Line[]) {
  const l = said.find((l) => l.kind === 'end' && l.playerId !== null)
  if (!l || l.side === null) return null
  const team = l.side === 0 ? view.home : view.away
  const p = [...team.players, ...team.out].find((p) => p.id === l.playerId)
  return p ? { player: p, club: team.club } : null
}

export const pct = (ok: number, all: number) => (all > 0 ? `${Math.round((100 * ok) / all)}%` : '-')

export const fullName = (p: LivePlayer) => p.listName.split(', ').reverse().join(' ')

/** A cor de uma nota (1–10): verde das boas, amarela das médias, vermelha das más. */
export const ratingColour = (r: number | null) =>
  r === null ? '#5a5a6a' : r >= 8 ? 'rgb(var(--c-mint))' : r >= 7 ? '#b6e35a' : r >= 6 ? '#f2d24b' : r >= 5 ? '#f29a3b' : '#ff5a5a'

/** Aba da partida: escura e arredondada; a selecionada com borda e texto amarelos, como no CM. */
export const matchTab = (selected: boolean) =>
  `flex h-8 min-w-0 flex-1 items-center justify-center truncate rounded px-2 ${
    selected
      ? 'bg-cm-hover font-bold text-cm-yellow ring-1 ring-inset ring-cm-yellow'
      : 'bg-cm-raised text-cm-muted hover:bg-cm-hover hover:text-white'
  }`

export type Pane =
  | 'panorama'
  | 'stats'
  | 'zones'
  | 'report'
  | 'homeData'
  | 'ratings'
  | 'results'
  | 'table'
  | 'awayData'
  | 'analysis'
  | 'post'
