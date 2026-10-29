import { Trophy } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { api } from '../api'
import type { CompetitionListItem } from '../bindings/CompetitionListItem'
import PickList, { type PickGroup } from '../components/PickList'
import { BottomBar, ErrorBox, ScreenHeader, Tabs } from '../components/Screen'
import { CONTINENTS, rememberTab } from '../continents'
import { useEscBack, useNav } from '../nav'

const TABS = [...CONTINENTS.map((c) => ({ key: String(c.id), label: c.label })), { key: 'world', label: 'Mundial' }]

/** Tela "Competições": competições por continente, as continentais primeiro e depois por nação. */
export default function CompetitionsPage() {
  const nav = useNav()
  useEscBack()
  const [all, setAll] = useState<CompetitionListItem[] | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [tab, setTab] = useState(rememberTab('competitions') ?? '5')

  useEffect(() => {
    api.listCompetitions().then(setAll, (e) => setError(String(e)))
  }, [])

  const groups: PickGroup[] = useMemo(() => {
    const toItem = (c: CompetitionListItem) => ({ id: c.id, label: c.name })
    if (tab === 'world') {
      return [{ items: (all ?? []).filter((c) => c.nationId === null && c.continentId === null).map(toItem) }]
    }
    const continent = Number(tab)
    const inContinent = (all ?? []).filter((c) => c.continentId === continent)
    const byNation = new Map<string, CompetitionListItem[]>()
    for (const c of inContinent.filter((c) => c.nation)) {
      byNation.set(c.nation!, [...(byNation.get(c.nation!) ?? []), c])
    }
    return [
      { title: 'Continentais', items: inContinent.filter((c) => c.nationId === null).map(toItem) },
      ...[...byNation.entries()].map(([nation, comps]) => ({ title: nation, items: comps.map(toItem) })),
    ]
  }, [all, tab])

  return (
    <div className="flex h-full flex-col">
      <ScreenHeader icon={<Trophy size={22} />} title="Competições" />
      <Tabs
        tabs={TABS}
        active={tab}
        onChange={(t) => {
          setTab(t)
          rememberTab('competitions', t)
        }}
      />
      <div className="mx-1.5 mt-1.5 flex min-h-0 flex-1 flex-col border border-cm-line bg-cm-panel/80">
        <ErrorBox>{error}</ErrorBox>
        {all && <PickList key={tab} groups={groups} onOpen={(c) => nav.go({ kind: 'competition', id: c.id })} />}
      </div>
      <BottomBar onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />
    </div>
  )
}
