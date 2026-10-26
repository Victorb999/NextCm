import { useEffect, useState } from 'react'
import { api } from '../api'
import type { CareerClub } from '../bindings/CareerClub'
import { BottomBar, Link, TitleBand } from '../components/Screen'
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
      <TitleBand title={`Currículo de ${game.info?.managerName ?? ''}`} fg="#000000" bg="#f4f4f4" />
      <div className="mx-1.5 mt-1.5 min-h-0 flex-1 space-y-2 overflow-auto border border-cm-line bg-cm-panel/70 p-3">
        {clubs.map((c, i) => (
          <div key={`${c.club.id}-${i}`} className="border border-cm-line bg-black/50 px-3 py-2">
            <div className="flex items-baseline justify-between">
              <Link className="font-title text-[20px] font-bold text-cm-yellow" onClick={() => nav.go({ kind: 'club', id: c.club.id })}>
                {c.club.name}
              </Link>
              <span className="text-[14px] text-white">
                {cmDate(c.joined)} – {c.left ? cmDate(c.left) : 'actual'}
              </span>
            </div>
            <div className="mt-1 grid grid-cols-3 gap-2 text-[15px]">
              <span className="text-white">
                Compras: <b className="text-cm-orange">{c.purchases}</b> ({money(c.spent) || '€0'})
              </span>
              <span className="text-white">
                Vendas: <b className="text-cm-orange">{c.sales}</b> ({money(c.received) || '€0'})
              </span>
              <span className="text-white">
                Dispensas: <b className="text-cm-orange">{c.releases}</b>
              </span>
            </div>
            <div className="mt-1 text-[15px] text-white">
              Títulos: <span className="text-cm-yellow">{c.titles.length ? c.titles.join(', ') : 'Nenhum'}</span>
            </div>
          </div>
        ))}
        {clubs.length === 0 && <div className="p-6 text-center text-cm-magenta">Sem clubes no currículo.</div>}
      </div>
      <BottomBar onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />
    </div>
  )
}
