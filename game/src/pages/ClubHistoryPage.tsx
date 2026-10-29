import { useEffect, useState } from 'react'
import { api } from '../api'
import type { ClubProfile } from '../bindings/ClubProfile'
import { BottomBar, Empty, ErrorBox, Link, ScreenHeader, Tabs } from '../components/Screen'
import { useEscBack, useNav } from '../nav'

const TABS = [
  { key: 'competitions', label: 'Competições' },
  { key: 'milestones', label: 'Marcos Significativos', disabled: true },
  { key: 'records', label: 'Registos', disabled: true },
  { key: 'positions', label: 'Classificações' },
  { key: 'attendance', label: 'Assistências', disabled: true },
]

const ordinal = (n: number) => `${n}º`

function Competitions({ c }: { c: ClubProfile }) {
  const nav = useNav()
  if (c.honours.length === 0) return <Empty>Nenhum título registado.</Empty>
  return (
    <div className="min-h-0 flex-1 overflow-auto px-2 pb-2">
      <div className="py-1.5 text-center font-title text-xl font-bold text-cm-yellow">Títulos</div>
      {c.honours.map((h) => (
        <div key={h.competition.id} className="mb-1 border-b border-cm-line bg-black/40 px-2 py-1">
          <div className="flex items-center gap-2">
            <Link onClick={() => nav.go({ kind: 'competition', id: h.competition.id })} className="flex-1 truncate font-title text-[17px] text-white">
              {h.competition.name}
            </Link>
            {h.won.length > 0 && <span className="font-bold text-cm-yellow">{h.won.length}× campeão</span>}
            {h.runnerUp.length > 0 && <span className="text-[#aab3c4]">{h.runnerUp.length}× vice</span>}
          </div>
          {h.won.length > 0 && <div className="text-[13px] text-cm-yellow">Campeão: {h.won.join(', ')}</div>}
          {h.runnerUp.length > 0 && <div className="text-[13px] text-[#aab3c4]">Vice: {h.runnerUp.join(', ')}</div>}
        </div>
      ))}
    </div>
  )
}

function Positions({ c }: { c: ClubProfile }) {
  const nav = useNav()
  if (!c.lastDivision || !c.lastPosition) return <Empty>Sem classificações registadas.</Empty>
  return (
    <div className="mx-auto mt-4 w-full max-w-2xl px-1.5">
      <div className="py-1.5 text-center font-title text-xl font-bold text-cm-yellow">Recente</div>
      <div className="flex h-8 items-center gap-3 border-b border-cm-line bg-black/50 px-3 text-[15px]">
        {/* O patch guarda só a posição "anterior", sem dizer de que ano */}
        <span className="w-32 text-white">Época anterior</span>
        <Link onClick={() => nav.go({ kind: 'competition', id: c.lastDivision!.id })} className="flex-1 text-cm-yellow">
          {c.lastDivision.name}
        </Link>
        <span className="font-bold text-cm-yellow">{ordinal(c.lastPosition)}</span>
      </div>
    </div>
  )
}

/** Tela "Historial <clube>": títulos (do historial das competições) e a última classificação. */
export default function ClubHistoryPage({ id }: { id: number }) {
  const nav = useNav()
  useEscBack()
  const [club, setClub] = useState<ClubProfile | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [tab, setTab] = useState('competitions')

  useEffect(() => {
    api.getClub(id).then(setClub, (e) => setError(String(e)))
  }, [id])


  return (
    <div className="flex h-full flex-col">
      <ScreenHeader club={club?.club} eyebrow={club?.club.name} title="Historial" />
      <Tabs tabs={TABS} active={tab} onChange={setTab} />
      <div className="mx-1.5 mt-1.5 flex min-h-0 flex-1 flex-col border border-cm-line bg-cm-panel/80">
        <ErrorBox>{error}</ErrorBox>
        {club && tab === 'competitions' && <Competitions c={club} />}
        {club && tab === 'positions' && <Positions c={club} />}
      </div>
      <div className="mt-1.5">
        <Tabs
          tabs={[
            { key: 'results', label: 'Resultados', disabled: true },
            { key: 'runs', label: 'Sequências', disabled: true },
            { key: 'players', label: 'Jogadores', disabled: true },
            { key: 'transfers', label: 'Transferências', disabled: true },
          ]}
          active=""
          onChange={() => {}}
        />
      </div>
      <BottomBar onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />
    </div>
  )
}
