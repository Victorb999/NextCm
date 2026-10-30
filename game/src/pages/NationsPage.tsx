import { Globe } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { api } from '../api'
import type { NationListItem } from '../bindings/NationListItem'
import PickList from '../components/PickList'
import { BottomBar, ErrorBox, ScreenHeader, Tabs } from '../components/Screen'
import { CONTINENTS, rememberTab } from '../continents'
import { useEscBack, useNav } from '../nav'

const TABS = CONTINENTS.map((c) => ({ key: String(c.id), label: c.label }))

/** Tela "Nações & Clubes": escolha da nação por continente. */
export default function NationsPage() {
  const nav = useNav()
  useEscBack()
  const [all, setAll] = useState<NationListItem[] | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [tab, setTab] = useState(rememberTab('nations') ?? '5')

  useEffect(() => {
    api.listNationsFull().then(setAll, (e) => setError(String(e)))
  }, [])

  const items = useMemo(
    () =>
      (all ?? [])
        .filter((n) => n.continentId === Number(tab))
        .map((n) => ({ id: n.id, label: n.name, detail: `${n.clubs} clubes` })),
    [all, tab],
  )

  return (
    <div className="flex h-full flex-col">
      <ScreenHeader icon={<Globe size={22} />} title="Nações & Clubes" />
      <Tabs
        tabs={TABS}
        active={tab}
        onChange={(t) => {
          setTab(t)
          rememberTab('nations', t)
        }}
      />
      <div className="mx-2 mt-2 flex min-h-0 flex-1 flex-col rounded-md bg-[#1a1b21]">
        <ErrorBox>{error}</ErrorBox>
        {all && <PickList key={tab} groups={[{ items }]} onOpen={(n) => nav.go({ kind: 'nation', id: n.id })} />}
      </div>
      <BottomBar onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />
    </div>
  )
}
