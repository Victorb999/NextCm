import { ScrollText } from 'lucide-react'
import { useEffect, useState } from 'react'
import { api } from '../api'
import type { CareerClub } from '../bindings/CareerClub'
import type { ManagerStats } from '../bindings/ManagerStats'
import { BottomBar, Empty, Link, Panel, ScreenHeader } from '../components/Screen'
import { cmDate, money } from '../format'
import { useGame } from '../game'
import { useEscBack, useNav } from '../nav'

/** "Currículo" do treinador, como no CM: os clubes, com as compras, as vendas, as dispensas e os títulos. */
export default function CareerPage() {
  const nav = useNav()
  const game = useGame()
  useEscBack()
  const [clubs, setClubs] = useState<CareerClub[]>([])
  const [stats, setStats] = useState<ManagerStats | null>(null)
  useEffect(() => {
    api.managerCareer().then(setClubs, () => setClubs([]))
    api.managerStats().then(setStats, () => setStats(null))
  }, [game.info?.date])
  return (
    <div className="flex h-full flex-col">
      <ScreenHeader icon={<ScrollText size={22} />} eyebrow={game.info?.managerName} title="Currículo" />
      {stats && (
        <Panel title="Estatísticas" className="mx-2 mb-2 shrink-0">
          <div className="cm-zebra text-[14px]">
            <div className="flex h-7 items-center gap-4 px-3">
              <span className="w-44 text-cm-muted">Reputação</span>
              <span className="font-bold text-cm-yellow">{stats.level}</span>
            </div>
            <div className="flex h-7 items-center gap-4 px-3">
              <span className="w-44 text-cm-muted">Qualificações</span>
              <span className="font-bold text-cm-yellow">{stats.license}</span>
            </div>
            {stats.nations.map((n) => (
              <div key={n.nation} className="flex h-7 items-center gap-4 px-3">
                <span className="w-44 truncate text-cm-muted">{n.nation}</span>
                <span className="w-28 font-bold text-cm-yellow">{n.level}</span>
                <span className="text-[13px] text-cm-muted">
                  {n.years.toLocaleString('pt-BR')} {n.years === 1 ? 'ano' : 'anos'}
                  {n.titles > 0 && ` · ${n.titles} ${n.titles === 1 ? 'título' : 'títulos'}`}
                </span>
              </div>
            ))}
          </div>
        </Panel>
      )}
      <Panel title="Clubes" className="mx-2 mb-1 min-h-0 flex-1">
        <div className="min-h-0 flex-1 overflow-auto">
          {clubs.length === 0 ? (
            <Empty>Sem clubes no currículo.</Empty>
          ) : (
            <table className="w-full border-collapse text-[14px]">
              <thead className="sticky top-0">
                <tr className="h-7">
                  {['Clube', 'Período', 'Compras', 'Vendas', 'Dispensas', 'Títulos'].map((h, i) => (
                    <th key={h} className={`bg-cm-headdeep px-3 font-mono text-[11px] font-bold uppercase tracking-[0.1em] text-cm-yellow ${i === 0 || i === 5 ? 'text-left' : 'text-center'}`}>
                      {h}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {clubs.map((c, i) => (
                  <tr key={`${c.club.id}-${i}`} className="h-7">
                    <td className="px-3">
                      <Link className="font-semibold text-cm-yellow" onClick={() => nav.go({ kind: 'club', id: c.club.id })}>
                        {c.club.name}
                      </Link>
                    </td>
                    <td className="px-3 text-center text-white">
                      {cmDate(c.joined)} – {c.left ? cmDate(c.left) : 'actual'}
                    </td>
                    <td className="px-3 text-center tabular-nums text-cm-orange">
                      {c.purchases} <span className="text-cm-dim">({money(c.spent) || '€0'})</span>
                    </td>
                    <td className="px-3 text-center tabular-nums text-cm-orange">
                      {c.sales} <span className="text-cm-dim">({money(c.received) || '€0'})</span>
                    </td>
                    <td className="px-3 text-center tabular-nums text-cm-orange">{c.releases}</td>
                    <td className="px-3 text-cm-yellow">{c.titles.length ? c.titles.join(', ') : '-'}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </Panel>
      <BottomBar onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />
    </div>
  )
}
