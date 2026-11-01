import type { SquadPlayer } from './bindings/SquadPlayer'
import type { MenuOption } from './components/Screen'
import { shortListName } from './format'

// Partilhado pelas vistas do Plantel (grade Tradicional e tabelas do Vêr).

export type SquadView = 'traditional' | 'contract' | 'selection' | 'stats' | 'more' | 'attributes' | 'others' | 'penalties'

export const VIEW_TITLE: Record<SquadView, string> = {
  traditional: 'Tradicional',
  contract: 'Informação sobre o Contrato',
  selection: 'Selecção - Informação',
  stats: 'Estatísticas',
  more: 'Estatísticas',
  attributes: 'Características',
  others: 'Outra Informação',
  penalties: 'Marcadores Penalties',
}

/** Cor do nome, como no CM: emprestado (por ou a outro clube) a azul, sem contrato ou de
 * enchimento a roxo. */
export function nameClass(p: SquadPlayer) {
  return p.ghost || p.free ? 'text-[#9b6fe8]' : p.loaned || p.loanedOut ? 'text-[#3aa0ff]' : 'text-white'
}

/** O "Competições" do Vêr Mais Dados (o `kind` do core). */
export const MORE_COMPETITIONS: { key: string; label: string }[] = [
  { key: '', label: 'Jogos Oficiais' },
  { key: 'Liga', label: 'Liga' },
  { key: 'Taça', label: 'Taça' },
  { key: 'Continentais', label: 'Continentais' },
]

/** Os grupos do Vêr Atributos (plantel e Treinos): o título e as colunas (atributo, sigla). */
export type AttributeGroup = 'physical' | 'technical' | 'mental' | 'goalkeeping'
export const ATTRIBUTE_GROUPS: { key: AttributeGroup; label: string; title: string; columns: [string, string][] }[] = [
  {
    key: 'physical',
    label: 'Físicos',
    title: 'Características Físicas',
    columns: [
      ['acceleration', 'Ace'],
      ['agility', 'Agi'],
      ['balance', 'Bal'],
      ['heading', 'Cab'],
      ['strength', 'For'],
      ['jumping', 'Imp'],
      ['stamina', 'Res'],
      ['technique', 'Téc'],
      ['pace', 'Vel'],
    ],
  },
  {
    key: 'technical',
    label: 'Técnicos',
    title: 'Características Técnicas',
    columns: [
      ['crossing', 'Cru'],
      ['dribbling', 'Fta'],
      ['finishing', 'Fin'],
      ['long_shots', 'RLo'],
      ['passing', 'Pas'],
      ['free_kicks', 'BPa'],
      ['tackling', 'Des'],
      ['marking', 'Mar'],
      ['off_the_ball', 'SBo'],
    ],
  },
  {
    key: 'mental',
    label: 'Mentais',
    title: 'Características Mentais',
    columns: [
      ['aggression', 'Agr'],
      ['anticipation', 'Ant'],
      ['bravery', 'Bra'],
      ['vision', 'Cri'],
      ['decisions', 'Dec'],
      ['determination', 'Det'],
      ['flair', 'Ins'],
      ['leadership', 'Ifl'],
      ['positioning', 'Pos'],
      ['teamwork', 'TEq'],
      ['work_rate', 'ITr'],
    ],
  },
  {
    key: 'goalkeeping',
    label: 'Guarda-Redes',
    title: 'Características de Guarda-Redes',
    columns: [
      ['handling', 'JMã'],
      ['reflexes', 'Ref'],
      ['one_on_ones', 'UxU'],
      ['positioning', 'Pos'],
      ['anticipation', 'Ant'],
      ['decisions', 'Dec'],
    ],
  },
]

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
