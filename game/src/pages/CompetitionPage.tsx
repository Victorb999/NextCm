import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { api } from '../api'
import type { ClubRef } from '../bindings/ClubRef'
import type { CompetitionProfile } from '../bindings/CompetitionProfile'
import PickList from '../components/PickList'
import { BottomBar, Link, Tabs, TitleBand } from '../components/Screen'
import { continentPt } from '../continents'
import { useEscBack, useNav } from '../nav'

const TABS = [
  { key: 'clubs', label: 'Clubes' },
  { key: 'table', label: 'Classificação', disabled: true },
  { key: 'fixtures', label: 'Calendário', disabled: true },
  { key: 'stats', label: 'Estatísticas', disabled: true },
  { key: 'history', label: 'Historial' },
]

function ClubLink({ club }: { club: ClubRef | null }) {
  const nav = useNav()
  if (!club) return <span className="text-[#666]">-</span>
  return (
    <Link onClick={() => nav.go({ kind: 'club', id: club.id })} className="text-cm-magenta">
      {club.shortName}
    </Link>
  )
}

function History({ c }: { c: CompetitionProfile }) {
  if (c.history.length === 0) return <div className="p-6 text-center text-cm-magenta">Sem historial registado.</div>
  const head = ['Ano', 'Campeão', 'Vice', '3º']
  return (
    <div className="min-h-0 flex-1 overflow-auto px-1.5 pt-2">
      <table className="w-full border-collapse">
        <thead className="sticky top-0">
          <tr className="h-6">
            {head.map((h) => (
              <th key={h} className="border border-black bg-cm-head px-2 text-left font-normal text-white">
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {c.history.map((h, i) => (
            <tr key={i} className="h-[24px] border-b border-cm-line bg-black/40 text-[15px]">
              <td className="w-20 px-2 tabular-nums text-cm-yellow">{h.year ?? '-'}</td>
              <td className="px-2">
                <ClubLink club={h.winner} />
              </td>
              <td className="px-2">
                <ClubLink club={h.runnerUp} />
              </td>
              <td className="px-2">
                <ClubLink club={h.third} />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

export default function CompetitionPage({ id }: { id: number }) {
  const nav = useNav()
  useEscBack()
  const [comp, setComp] = useState<CompetitionProfile | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [tab, setTab] = useState('clubs')

  useEffect(() => {
    api.getCompetition(id).then(
      (c) => {
        setComp(c)
        if (c.clubs.length === 0 && c.history.length > 0) setTab('history')
      },
      (e) => setError(String(e)),
    )
  }, [id])

  const where: ReactNode = comp?.nation ? (
    <Link onClick={() => nav.go({ kind: 'nation', id: comp.nationId! })}>{comp.nation}</Link>
  ) : (
    continentPt(comp?.continent ?? 'Mundial')
  )

  return (
    <div className="flex h-full flex-col">
      <TitleBand title={comp?.name ?? ''} fg="#ffffff" bg="#0b2a78" />
      <Tabs tabs={TABS} active={tab} onChange={setTab} />
      <div className="mx-1.5 mt-1.5 flex min-h-0 flex-1 flex-col border border-cm-line bg-cm-panel/80">
        {error && <div className="bg-red-900 px-2 py-1 text-white">{error}</div>}
        {comp && (
          <div className="shrink-0 pt-1.5 text-center text-cm-magenta">
            {where} · Reputação {comp.reputation}
            {comp.kind === 'national' && ' · Seleções'}
          </div>
        )}
        {comp && tab === 'clubs' && (
          <>
            <div className="shrink-0 py-1 text-center font-title text-xl font-bold text-cm-yellow">
              {comp.clubs.length} clubes
            </div>
            <PickList
              groups={[
                {
                  items: comp.clubs.map((c) => ({
                    id: c.club.id,
                    label: c.club.shortName,
                    // Nação só faz sentido em competições continentais/mundiais
                    detail: comp.nationId === null ? (c.nation ?? '') : undefined,
                  })),
                },
              ]}
              onOpen={(c) => nav.go({ kind: 'club', id: c.id })}
              empty="Sem lista de participantes (definida a cada temporada)."
            />
          </>
        )}
        {comp && tab === 'history' && <History c={comp} />}
      </div>
      <BottomBar onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />
    </div>
  )
}
