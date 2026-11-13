import type { ReactNode } from 'react'
import type { PersonProfile } from '../../bindings/PersonProfile'
import { Link, Panel } from '../../components/Screen'
import { useNav } from '../../nav'

/** Informação pessoal, como a do FM08: preferências, personalidade, felicidade e relações. */
export default function PersonalPanel({ p }: { p: PersonProfile }) {
  const nav = useNav()
  const info = p.personal!
  const clubs = (list: typeof info.favouredClubs) =>
    list.length
      ? list.map((c, i) => (
          <span key={c.id}>
            {i > 0 && ', '}
            <Link onClick={() => nav.go({ kind: 'club', id: c.id })}>{c.shortName}</Link>
          </span>
        ))
      : '-'
  const people = (list: typeof info.favouredPeople) =>
    list.length
      ? list.map((x, i) => (
          <span key={x.id}>
            {i > 0 && ', '}
            <Link onClick={() => nav.go({ kind: 'person', id: x.id })}>{x.name}</Link>
          </span>
        ))
      : '-'
  const rows: [string, ReactNode][] = [
    ['Personalidade', info.personality],
    ['Reputação', info.reputation],
    ['Clubes favoritos', clubs(info.favouredClubs)],
    ['Clubes detestados', clubs(info.dislikedClubs)],
    ['Pessoas favoritas', people(info.favouredPeople)],
    ['Pessoas detestadas', people(info.dislikedPeople)],
    ['Felicidade', info.happiness],
    ['Planos', info.plans],
    ['Relações', info.relationships.length ? info.relationships.join('; ') : '-'],
  ]
  return (
    <Panel title="Informação Pessoal">
      <div className="cm-zebra">
        {rows.map(([k, v]) => (
          <div key={k} className="flex min-h-6 items-start gap-3 px-3 py-0.5 text-[13.5px]">
            <span className="w-36 shrink-0 text-cm-muted">{k}</span>
            <span className="min-w-0 flex-1 text-right font-bold text-cm-yellow">{v}</span>
          </div>
        ))}
      </div>
    </Panel>
  )
}
