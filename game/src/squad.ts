import type { SquadPlayer } from './bindings/SquadPlayer'
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
