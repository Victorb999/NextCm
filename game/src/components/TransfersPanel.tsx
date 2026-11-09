import { useEffect, useState } from 'react'
import { api } from '../api'
import type { ClubMoves } from '../bindings/ClubMoves'
import { cmDate, money } from '../format'
import { useGame } from '../game'
import { useNav } from '../nav'
import { Empty, ErrorBox, Link, PurpleMenu, type MenuOption } from './Screen'

// A aba Transferências do clube, como a do CM: « Temporada / Temporada », o menu Ver com as sete
// vistas e a lista (data ou "Oferecer", o jogador, o outro clube e o valor), com o saldo no fundo.
// Tudo vem do core (`club_moves`).

type View = 'in' | 'out' | 'future' | 'loan_in' | 'loan_out' | 'staff_in' | 'staff_out'

const VIEWS: MenuOption<View>[] = [
  { key: 'in', label: 'Entradas Jogadores' },
  { key: 'out', label: 'Saídas Jogadores' },
  { key: 'future', label: 'Transferências Futuras' },
  { key: 'loan_in', label: 'Emprest. Ao Clube' },
  { key: 'loan_out', label: 'Emprest. Pelo Clube' },
  { key: 'staff_in', label: 'Entradas Staff' },
  { key: 'staff_out', label: 'Saídas Staff' },
]

export default function TransfersPanel({ clubId }: { clubId: number }) {
  const game = useGame()
  const nav = useNav()
  const [view, setView] = useState<View>('in')
  const [season, setSeason] = useState<number | null>(null)
  const [moves, setMoves] = useState<ClubMoves | null>(null)
  const [error, setError] = useState<string | null>(null)
  const date = game.info?.date

  useEffect(() => {
    api.clubMoves(clubId, view, season).then(
      (m) => {
        setMoves(m)
        setError(null)
      },
      (e) => setError(String(e)),
    )
  }, [clubId, view, season, date])

  const pick = (v: View) => {
    setView(v)
    setSeason(null)
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col pb-2">
      <div className="flex h-9 shrink-0 items-center gap-1.5 px-1">
        <button className="cm-purple h-6 w-36" disabled={moves?.prev == null} onClick={() => setSeason(moves!.prev)}>
          {'<< Temporada'}
        </button>
        <button className="cm-purple h-6 w-36" disabled={moves?.next == null} onClick={() => setSeason(moves!.next)}>
          {'Temporada >>'}
        </button>
        <div className="ml-auto">
          <PurpleMenu label="Ver" options={VIEWS} value={view} onChange={pick} align="right" />
        </div>
      </div>
      <ErrorBox className="mb-1">{error}</ErrorBox>
      <div className="shrink-0 py-1 text-center font-title text-[20px] text-cm-yellow">{moves?.title ?? ''}</div>
      <div className="cm-zebra min-h-0 flex-1 overflow-auto">
        {moves && moves.rows.length === 0 && <Empty>Nenhuma.</Empty>}
        {moves?.rows.map((r, i) => (
          <div key={`${r.personId}-${i}`} className="flex h-7 items-center gap-3 px-3 text-[15px]">
            <span className="cm-cell w-28 shrink-0 text-center text-[13px] text-white">
              {r.when === 'Oferecer' ? 'Oferecer' : cmDate(r.when)}
            </span>
            <span className="min-w-0 flex-1 truncate">
              <Link className="font-title text-[17px] text-white" onClick={() => nav.go({ kind: 'person', id: r.personId })}>
                {r.name}
              </Link>
            </span>
            <span className="w-64 shrink-0 truncate text-cm-yellow">
              {r.club ? <Link onClick={() => nav.go({ kind: 'club', id: r.club!.id })}>{r.club.name}</Link> : '-'}
            </span>
            <span className="cm-cell w-32 shrink-0 text-center text-[13px] text-white">{r.value}</span>
          </div>
        ))}
      </div>
      {moves?.total != null && (
        <div className="flex h-7 shrink-0 items-center justify-end px-3">
          <span className={`cm-cell w-32 text-center text-[13px] font-bold ${moves.total < 0 ? 'text-[#4ee6ff]' : 'text-cm-yellow'}`}>
            {moves.total < 0 ? `-${money(-moves.total)}` : money(moves.total)}
          </span>
        </div>
      )}
    </div>
  )
}
