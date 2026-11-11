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
  return p.ghost || p.free ? 'text-cm-purplelight' : p.loaned || p.loanedOut ? 'text-[#3aa0ff]' : 'text-white'
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

/** A ordem da moral, da melhor para a pior. */
export const MORALE_ORDER = ['Excelente', 'Muito Boa', 'Boa', 'Razoável', 'Fraca', 'Muito Fraca', 'Péssima']
export const moraleRank = (p: SquadPlayer) => (p.morale ? MORALE_ORDER.indexOf(p.morale) : null)
export const formAverage = (p: SquadPlayer) => (p.form.length ? p.form.reduce((s, x) => s + x, 0) / p.form.length : null)

/** O valor por que uma coluna se ordena; `desc`: do maior para o menor (os números do jogo). */
export type SortValue = (p: SquadPlayer) => number | string | null | undefined
export type Sortable = { sort?: SortValue; desc?: boolean }
/** A ordem pelo cabeçalho clicado: o rótulo da coluna e se vai ao contrário. */
export type HeaderSort = { label: string; reverse: boolean }

/** As colunas fixas das listas do plantel (as outras trazem a sua ordem). */
export const FIXED_SORTS: Record<string, Sortable> = {
  Nome: { sort: (p) => p.listName },
  Jogador: { sort: (p) => p.listName },
  Nº: { sort: (p) => p.squadNumber },
  Posição: { sort: (p) => p.positionOrder },
}

/** Ordena pela coluna, com os sem valor no fim e os empates pelo nome. */
export function sortByColumn(list: SquadPlayer[], col: Sortable, reverse: boolean) {
  const value = col.sort
  if (!value) return list
  const dir = (col.desc ? -1 : 1) * (reverse ? -1 : 1)
  return list.sort((a, b) => {
    const x = value(a) ?? null
    const y = value(b) ?? null
    if (x === null || y === null) return x === y ? a.listName.localeCompare(b.listName) : x === null ? 1 : -1
    const c = typeof x === 'string' ? x.localeCompare(String(y)) : x - Number(y)
    return c * dir || a.listName.localeCompare(b.listName)
  })
}

/** O rótulo de um cabeçalho com a seta da ordem, se for o clicado. */
export const sortLabel = (label: string, s: HeaderSort | null, col: Sortable | undefined) =>
  s?.label === label && col?.sort ? `${label} ${!!col.desc !== s.reverse ? '▼' : '▲'}` : label

/** "Apelido, N" e o `*` dos jogadores-chave. */
export function squadName(p: SquadPlayer) {
  return `${shortListName(p.listName)}${p.key ? '*' : ''}`
}

/** O Filtrar: posição, lado, disponibilidade, (nas reservas) incluir os séniores e (no plantel
 * principal) incluir as reservas, ligado como no CM. */
export type Filter = { line: string; side: string; avail: '' | 'yes' | 'no'; seniors: boolean; reserves: boolean }
export const NO_FILTER: Filter = { line: '', side: '', avail: '', seniors: false, reserves: true }

// O filtro do plantel fica gravado neste computador: trocar de tela e voltar mantém-no
const FILTER_KEY = 'nextcm.squadFilter'

export function loadFilter(): Filter {
  try {
    const raw = localStorage.getItem(FILTER_KEY)
    return raw ? { ...NO_FILTER, ...(JSON.parse(raw) as Partial<Filter>) } : NO_FILTER
  } catch {
    return NO_FILTER
  }
}

export function saveFilter(f: Filter) {
  try {
    localStorage.setItem(FILTER_KEY, JSON.stringify(f))
  } catch {
    // Sem localStorage: o filtro vale só nesta tela
  }
}

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
  // Os emprestados a outro clube e os que estão fora na selecção ("Int") também não estão disponíveis
  const out = p.injured || p.suspended || p.loanedOut || p.info === 'Int'
  return (
    (!f.line || p.lines.includes(f.line)) &&
    (!f.side || p.sides.includes(f.side)) &&
    (f.avail !== 'yes' || !out) &&
    (f.avail !== 'no' || out)
  )
}
