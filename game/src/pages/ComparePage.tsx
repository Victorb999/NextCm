import { Columns2 } from 'lucide-react'
import { useEffect, useState } from 'react'
import { api } from '../api'
import { PLAYER_ATTRIBUTES } from '../attributes'
import type { PersonProfile } from '../bindings/PersonProfile'
import { BottomBar, Link, ScreenHeader } from '../components/Screen'
import { money } from '../format'
import { useEscBack, useNav } from '../nav'

const ROW = 'grid grid-cols-[1fr_9rem_9rem] items-center gap-x-2 border-b border-cm-line text-[15px] tabular-nums'

/** "Comparar Jogadores": dois jogadores lado a lado (o que o clube conhece deles). */
export default function ComparePage({ a, b }: { a: number; b: number }) {
  const nav = useNav()
  useEscBack()
  const [people, setPeople] = useState<[PersonProfile, PersonProfile] | null>(null)
  useEffect(() => {
    Promise.all([api.getPerson(a), api.getPerson(b)]).then(([x, y]) => setPeople([x, y]), () => {})
  }, [a, b])
  // O melhor dos dois a verde
  const colour = (v: number | null | undefined, other: number | null | undefined) =>
    v == null ? 'text-[#777]' : other != null && v > other ? 'font-bold text-[#4cd964]' : 'text-cm-orange'
  return (
    <div className="flex h-full flex-col">
      <ScreenHeader icon={<Columns2 size={22} />} title="Comparar Jogadores" />
      {people && (
        <div className="mx-1.5 mt-1.5 min-h-0 flex-1 overflow-auto border border-cm-line bg-cm-panel/70 px-6 py-2">
          <div className={`${ROW} pb-1 font-title text-[18px]`}>
            <span />
            {people.map((p) => (
              <Link key={p.id} className="text-center text-cm-yellow" onClick={() => nav.go({ kind: 'person', id: p.id })}>
                {p.name}
              </Link>
            ))}
          </div>
          <div className={ROW}>
            <span className="text-white">Clube</span>
            {people.map((p) => (
              <span key={p.id} className="text-center text-cm-yellow">
                {p.club?.shortName ?? 'Sem clube'}
              </span>
            ))}
          </div>
          <div className={ROW}>
            <span className="text-white">Idade</span>
            {people.map((p) => (
              <span key={p.id} className="text-center text-cm-yellow">
                {p.age ?? '-'}
              </span>
            ))}
          </div>
          <div className={ROW}>
            <span className="text-white">Valor</span>
            {people.map((p) => (
              <span key={p.id} className="text-center text-cm-yellow">
                {money(p.player?.value ?? 0)}
              </span>
            ))}
          </div>
          {PLAYER_ATTRIBUTES.map(([key, label]) => {
            const x = people[0].player?.attributes[key]
            const y = people[1].player?.attributes[key]
            return (
              <div key={key} className={ROW}>
                <span className="text-white">{label}</span>
                <span className={`text-center ${colour(x, y)}`}>{x ?? '-'}</span>
                <span className={`text-center ${colour(y, x)}`}>{y ?? '-'}</span>
              </div>
            )
          })}
        </div>
      )}
      <BottomBar onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />
    </div>
  )
}
