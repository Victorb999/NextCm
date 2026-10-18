import { useEffect, useMemo, useState } from 'react'
import { api } from '../api'
import type { NationListItem } from '../bindings/NationListItem'
import PickList from '../components/PickList'
import { BottomBar, Tabs, TitleBand } from '../components/Screen'
import { CONTINENTS, rememberTab } from '../continents'
import { useEscBack, useNav } from '../nav'

const TABS = CONTINENTS.map((c) => ({ key: String(c.id), label: c.label }))

/** Tela "Nações & Clubes": escolha da nação por continente. */
export default function NationsPage() {
  const nav = useNav()
  useEscBack()
  const [all, setAll] = useState<NationListItem[]>([])
  const [error, setError] = useState<string | null>(null)
  const [tab, setTab] = useState(rememberTab('nations') ?? '5')

  useEffect(() => {
    api.listNationsFull().then(setAll, (e) => setError(String(e)))
  }, [])

  const items = useMemo(
    () =>
      all
        .filter((n) => n.continentId === Number(tab))
        .map((n) => ({ id: n.id, label: n.name, detail: `${n.clubs} clubes` })),
    [all, tab],
  )

  return (
    <div className="flex h-full flex-col">
      <TitleBand title="Nações & Clubes" fg="#ffffff" bg="#1b6b2a" />
      <Tabs
        tabs={TABS}
        active={tab}
        onChange={(t) => {
          setTab(t)
          rememberTab('nations', t)
        }}
      />
      <div className="mx-1.5 mt-1.5 flex min-h-0 flex-1 flex-col border border-cm-line bg-cm-panel/80">
        {error && <div className="bg-red-900 px-2 py-1 text-white">{error}</div>}
        <PickList key={tab} groups={[{ items }]} onOpen={(n) => nav.go({ kind: 'nation', id: n.id })} />
      </div>
      <BottomBar onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />
    </div>
  )
}
