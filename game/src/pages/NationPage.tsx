import { Flag } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import type { ReactNode } from 'react'
import { api } from '../api'
import type { NationProfile } from '../bindings/NationProfile'
import PickList, { type PickGroup } from '../components/PickList'
import { BottomBar, ErrorBox, Link, ScreenHeader, Tabs } from '../components/Screen'
import { continentPt } from '../continents'
import { useEscBack, useNav } from '../nav'

const TABS = [
  { key: 'clubs', label: 'Clubes' },
  { key: 'competitions', label: 'Competições' },
  { key: 'info', label: 'Informação' },
]

function Info({ n }: { n: NationProfile }) {
  const nav = useNav()
  const rows: [string, ReactNode][] = [
    ['Continente', continentPt(n.continent)],
    [
      'Seleção',
      n.nationalTeam ? (
        <Link key="l1" onClick={() => nav.go({ kind: 'club', id: n.nationalTeam!.id })} className="text-cm-magenta">
          {n.nationalTeam.name}
        </Link>
      ) : (
        '-'
      ),
    ],
    ['Estádio nacional', n.stadium ?? '-'],
    ['Jogadores no banco', n.players.toLocaleString('pt-BR')],
    ['Clubes em competições', n.clubs.length],
    ['Clubes sem liga', n.otherClubs],
  ]
  return (
    <div className="cm-zebra">
      {rows.map(([k, v]) => (
        <div key={k} className="flex h-7 items-center gap-4 px-4 text-[14px]">
          <span className="w-56 font-semibold text-white">{k}</span>
          <span className="text-cm-yellow">{v}</span>
        </div>
      ))}
    </div>
  )
}

export default function NationPage({ id }: { id: number }) {
  const nav = useNav()
  useEscBack()
  const [nation, setNation] = useState<NationProfile | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [tab, setTab] = useState('clubs')

  useEffect(() => {
    api.getNation(id).then(setNation, (e) => setError(String(e)))
  }, [id])

  // Clubes agrupados por divisão, na ordem de reputação (as divisões mais fortes primeiro)
  const clubGroups: PickGroup[] = useMemo(() => {
    const groups = new Map<string, PickGroup>()
    for (const c of nation?.clubs ?? []) {
      const key = c.division ?? 'Sem divisão'
      if (!groups.has(key)) groups.set(key, { title: key, items: [] })
      groups.get(key)!.items.push({ id: c.club.id, label: c.club.shortName, detail: c.reputation ?? '' })
    }
    return [...groups.values()]
  }, [nation])

  return (
    <div className="flex h-full flex-col">
      <ScreenHeader icon={<Flag size={22} />} title={nation?.name ?? ''} />
      <Tabs tabs={TABS} active={tab} onChange={setTab} />
      <div className="mx-2 mt-2 flex min-h-0 flex-1 flex-col rounded-md bg-[#1a1b21]">
        <ErrorBox>{error}</ErrorBox>
        {nation && tab === 'clubs' && (
          <PickList key="clubs" groups={clubGroups} onOpen={(c) => nav.go({ kind: 'club', id: c.id })} />
        )}
        {nation && tab === 'competitions' && (
          <PickList
            key="competitions"
            groups={[{ items: nation.competitions.map((c) => ({ id: c.id, label: c.name })) }]}
            onOpen={(c) => nav.go({ kind: 'competition', id: c.id })}
            empty="Nenhuma competição nacional no banco."
          />
        )}
        {nation && tab === 'info' && <Info n={nation} />}
      </div>
      <BottomBar onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />
    </div>
  )
}
