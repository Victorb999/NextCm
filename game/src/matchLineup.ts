import type { LivePlayer } from './bindings/LivePlayer'
import type { LiveTeam } from './bindings/LiveTeam'
import type { SheetLine } from './bindings/SheetLine'

// A lista de jogadores de uma partida como as "Classificações Jogadores" do CM: o onze
// inicial pela ordem das funções (quem saiu fica no lugar dele, com "sub 60"), depois quem
// entrou ("ent 60") e os suplentes que não jogaram (a roxo).

export type LineupEntry = {
  id: number
  name: string
  number: number | null
  rating: number | null
  goals: number
  yellow: boolean
  red: boolean
  /** Saiu (substituído) a este minuto. */
  off: number | null
  /** Entrou do banco a este minuto. */
  on: number | null
  played: boolean
}

const fromLive = (p: LivePlayer): LineupEntry => ({
  id: p.id,
  name: p.listName,
  number: p.squadNumber,
  rating: p.rating,
  goals: p.goals,
  yellow: p.yellow,
  red: p.red,
  off: p.red ? null : p.offMinute,
  on: p.onMinute !== null && p.onMinute > 0 ? p.onMinute : null,
  played: true,
})

/** A partida ao vivo: quem entrou volta a dar o lugar a quem saiu no onze inicial. */
export function liveLineup(team: LiveTeam): LineupEntry[] {
  const out = [...team.out]
  const starters: LineupEntry[] = []
  const entered: LineupEntry[] = []
  for (const p of team.players) {
    if (p.onMinute !== null && p.onMinute > 0) {
      // O titular que saiu quando ele entrou fica neste lugar
      const k = out.findIndex((o) => (o.onMinute === null || o.onMinute === 0) && o.offMinute === p.onMinute)
      entered.push(fromLive(p))
      const [back] = k >= 0 ? out.splice(k, 1) : []
      if (back) starters.push(fromLive(back))
    } else starters.push(fromLive(p))
  }
  // Os titulares que saíram sem ninguém no lugar (expulsos) e os suplentes que entraram e já saíram
  for (const o of out) (o.onMinute !== null && o.onMinute > 0 ? entered : starters).push(fromLive(o))
  const unused = team.bench.map((p) => ({ ...fromLive(p), played: false }))
  return [...starters, ...entered, ...unused]
}

/** A ficha de uma partida jogada (só quem jogou fica gravado). */
export function sheetLineup(lines: SheetLine[]): LineupEntry[] {
  return lines.map((l) => ({
    id: l.id,
    name: l.name,
    number: l.number,
    rating: Math.round(l.rating),
    goals: l.goals,
    yellow: l.yellow,
    red: l.red,
    off: l.red ? null : l.off,
    on: l.on !== null && l.on > 0 ? l.on : null,
    played: true,
  }))
}
