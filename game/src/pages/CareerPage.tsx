import { ScrollText } from 'lucide-react'
import { useEffect, useState } from 'react'
import { api } from '../api'
import type { CareerClub } from '../bindings/CareerClub'
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
  useEffect(() => {
    api.managerCareer().then(setClubs, () => setClubs([]))
  }, [game.info?.date])
  return (
    <div className="flex h-full flex-col">
      <ScreenHeader icon={<ScrollText size={22} />} eyebrow={game.info?.managerName} title="Currículo" />
      <Panel title="Clubes" className="mx-2 mb-1 min-h-0 flex-1">
        <div className="min-h-0 flex-1 overflow-auto">
          {clubs.length === 0 ? (
            <Empty>Sem clubes no currículo.</Empty>
          ) : (
            <table className="w-full border-collapse text-[14px]">
              <thead className="sticky top-0">
                <tr className="h-7">
                  {['Clube', 'Período', 'Compras', 'Vendas', 'Dispensas', 'Títulos'].map((h, i) => (
                    <th key={h} className={`bg-[#3d1a73] px-3 font-mono text-[11px] font-bold uppercase tracking-[0.1em] text-cm-yellow ${i === 0 || i === 5 ? 'text-left' : 'text-center'}`}>
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
                      {c.purchases} <span className="text-[#86869a]">({money(c.spent) || '€0'})</span>
                    </td>
                    <td className="px-3 text-center tabular-nums text-cm-orange">
                      {c.sales} <span className="text-[#86869a]">({money(c.received) || '€0'})</span>
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
