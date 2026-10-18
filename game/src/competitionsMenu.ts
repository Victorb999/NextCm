import type { CompetitionListItem } from './bindings/CompetitionListItem'
import type { MenuNode } from './components/CascadeMenu'

// Menu do botão "Competições": seleções, clubes intercontinentais e, por continente,
// as competições continentais e as nações (cada uma com as suas competições).

const CONTINENT_MENUS: { id: number; label: string }[] = [
  { id: 0, label: 'Africanas' },
  { id: 1, label: 'Asiáticas' },
  { id: 2, label: 'Europeias' },
  { id: 3, label: 'Norte Americanas' },
  { id: 4, label: 'Oceânicas' },
  { id: 5, label: 'Sul Americanas' },
]

const byReputation = (a: CompetitionListItem, b: CompetitionListItem) =>
  b.reputation - a.reputation || a.name.localeCompare(b.name)

export function competitionsMenu(
  all: CompetitionListItem[],
  open: (id: number) => void,
  openAll: () => void,
): MenuNode[] {
  const leaf = (c: CompetitionListItem): MenuNode => ({ label: c.name, onSelect: () => open(c.id) })
  const list = (cs: CompetitionListItem[]) => [...cs].sort(byReputation).map(leaf)

  const national = all.filter((c) => c.kind === 'national')
  const clubs = all.filter((c) => c.kind !== 'national')

  const continents: MenuNode[] = CONTINENT_MENUS.map(({ id, label }) => {
    const continental = clubs.filter((c) => c.nationId === null && c.continentId === id)
    const nations = new Map<string, CompetitionListItem[]>()
    for (const c of clubs.filter((c) => c.nationId !== null && c.continentId === id)) {
      nations.set(c.nation ?? '?', [...(nations.get(c.nation ?? '?') ?? []), c])
    }
    const nationNodes: MenuNode[] = [...nations.entries()]
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([nation, cs]) => ({ label: nation, children: list(cs) }))
    const children = [...list(continental), ...(continental.length && nationNodes.length ? ['separator' as const] : []), ...nationNodes]
    return { label, children }
  }).filter((n) => n.children.length > 0)

  return [
    { label: 'Internacionais', children: list(national) },
    { label: 'Internacionais-Clubes', children: list(clubs.filter((c) => c.nationId === null && c.continentId === null)) },
    ...continents,
    'separator',
    { label: 'Todas as Competições', onSelect: openAll },
  ]
}
