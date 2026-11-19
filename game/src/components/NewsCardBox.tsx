import { UserRound } from 'lucide-react'
import type { ReactNode } from 'react'
import type { NewsCard } from '../bindings/NewsCard'
import { money } from '../format'
import { useNav } from '../nav'
import NationFlag from './NationFlag'
import Shirt from './Shirt'

const num = (v: number | null) => (v === null ? '-' : String(v))

/** O quadro ao lado do texto da notícia, como no CM: a pessoa ou o clube de que ela fala. */
export default function NewsCardBox({ card }: { card: NewsCard }) {
  const nav = useNav()
  const club = card.club
  const rows: [string, ReactNode][] =
    card.kind === 'club'
      ? [
          ['Divisão', card.division ?? '-'],
          ['Posição', card.position === null ? '-' : `${card.position}º`],
          ['Gols Pró', num(card.goalsFor)],
          ['Gols Contra', num(card.goalsAgainst)],
          ['Público Médio', card.attendance === null ? '-' : card.attendance.toLocaleString('pt-BR')],
        ]
      : [
          [
            'Nac',
            <span key="nac" className="inline-flex items-center gap-1.5">
              {card.nation ?? '-'} <NationFlag name={card.nation} />
            </span>,
          ],
          ['Idade', num(card.age)],
          [card.kind === 'player' ? 'Posição' : 'Função', card.role ?? '-'],
          ['Clube', club?.shortName ?? 'Sem clube'],
          ...(card.kind === 'player'
            ? ([
                ['Valor', money(card.value)],
                ['Jogos', num(card.apps)],
                ['Gols', num(card.goals)],
                ['Nota Média', card.rating === null ? '-' : card.rating.toFixed(2)],
              ] as [string, ReactNode][])
            : []),
        ]
  const open = () => nav.go(card.kind === 'club' ? { kind: 'club', id: card.id } : { kind: 'person', id: card.id })
  return (
    <div className="w-60 shrink-0 self-start overflow-hidden rounded border border-cm-line bg-black/25 text-[14px]">
      <button
        className="block w-full truncate px-2 py-1 text-left font-bold hover:underline"
        style={{ background: club?.bg ?? '#3a3a3a', color: club?.fg ?? '#ffffff' }}
        title={card.name}
        onClick={open}
      >
        {card.name}
      </button>
      <div className="flex h-16 items-center justify-center border-b border-cm-line bg-black/20">
        {card.kind === 'club' ? (
          <Shirt bg={club?.bg ?? null} fg={club?.fg ?? null} size={52} />
        ) : (
          <UserRound size={52} className="text-cm-dim" />
        )}
      </div>
      <div className="cm-zebra">
        {rows.map(([label, value]) => (
          <div key={label} className="flex h-[22px] items-center justify-between gap-2 px-2">
            <span className="text-cm-muted">{label}</span>
            <span className="truncate text-right font-bold text-cm-mint">{value}</span>
          </div>
        ))}
      </div>
    </div>
  )
}
