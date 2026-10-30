import type { SquadPlayer } from './bindings/SquadPlayer'
import type { MenuOption } from './components/Screen'
import { shortListName } from './format'

// Partilhado pelas vistas do Plantel (grade Tradicional e tabelas do Vêr).

export type SquadView = 'traditional' | 'contract' | 'selection' | 'stats' | 'penalties'

export const VIEW_TITLE: Record<SquadView, string> = {
  traditional: 'Tradicional',
  contract: 'Informação sobre o Contrato',
  selection: 'Selecção - Informação',
  stats: 'Estatísticas',
  penalties: 'Marcadores Penalties',
}

/** Cor do nome: emprestado a azul-claro, sem contrato ou de enchimento a roxo. */
export function nameClass(p: SquadPlayer) {
  return p.ghost || p.free ? 'text-[#9b6fe8]' : p.loaned ? 'text-[#7cc7ff]' : 'text-white'
}

/** "Apelido, N" e o `*` dos jogadores-chave. */
export function squadName(p: SquadPlayer) {
  return `${shortListName(p.listName)}${p.key ? '*' : ''}`
}

/** O Filtrar: posição, lado, disponibilidade e (nas reservas) incluir os séniores. */
export type Filter = { line: string; side: string; avail: '' | 'yes' | 'no'; seniors: boolean }
export const NO_FILTER: Filter = { line: '', side: '', avail: '', seniors: false }

export const FILTER_OPTIONS: MenuOption<string>[] = [
  { key: 'line:', label: 'Todas Posições' },
  { key: 'line:GR', label: 'Guarda-Redes' },
  { key: 'line:D', label: 'Defesas' },
  { key: 'line:M', label: 'Médios' },
  { key: 'line:A', label: 'Atacantes' },
  'separator',
  { key: 'side:', label: 'Todos Lados' },
  { key: 'side:E', label: 'Lado Esquerdo' },
  { key: 'side:C', label: 'Central' },
  { key: 'side:D', label: 'Lado Direito' },
  'separator',
  { key: 'avail:yes', label: 'Disponíveis' },
  { key: 'avail:no', label: 'Indisponíveis' },
]

export function passes(p: SquadPlayer, f: Filter) {
  const out = p.injured || p.suspended
  return (
    (!f.line || p.lines.includes(f.line)) &&
    (!f.side || p.sides.includes(f.side)) &&
    (f.avail !== 'yes' || !out) &&
    (f.avail !== 'no' || out)
  )
}
