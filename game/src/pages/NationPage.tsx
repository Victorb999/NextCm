import { useEffect, useMemo, useState } from 'react'
import type { ReactNode } from 'react'
import { api } from '../api'
import type { NationProfile } from '../bindings/NationProfile'
import PickList, { type PickGroup } from '../components/PickList'
import { BottomBar, Link, Tabs, TitleBand } from '../components/Screen'
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
    <div className="mx-auto mt-4 w-full max-w-2xl px-1.5">
      {rows.map(([k, v]) => (
        <div key={k} className="flex h-8 items-center justify-between border-b border-cm-line bg-black/50 px-3 text-[15px]">
          <span className="text-white">{k}</span>
          <span className="font-bold text-cm-yellow">{v}</span>
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
      <TitleBand title={nation?.name ?? ''} fg="#ffffff" bg="#1b6b2a" />
      <Tabs tabs={TABS} active={tab} onChange={setTab} />
      <div className="mx-1.5 mt-1.5 flex min-h-0 flex-1 flex-col border border-cm-line bg-cm-panel/80">
        {error && <div className="bg-red-900 px-2 py-1 text-white">{error}</div>}
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
