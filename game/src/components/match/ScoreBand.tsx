import type { ClubRef } from '../../bindings/ClubRef'
import { initials } from '../../format'
import { clubBand } from './lines'

/** Faixa de um clube no resultado: emblema, nome grande nas cores do clube e o placar numa casa. */
export function ScoreBand({
  club,
  goals,
  side,
  detail,
  onClick,
}: {
  club: ClubRef
  goals: number | null
  side: 'home' | 'away'
  detail: string
  onClick?: () => void
}) {
  return (
    <div
      className={`flex h-16 min-w-0 flex-1 items-center gap-3 rounded px-3 shadow-lg ${side === 'away' ? 'flex-row-reverse' : ''} ${onClick ? 'cursor-pointer' : ''}`}
      style={clubBand(club)}
      onClick={onClick}
      title={onClick ? `Tácticas ${club.shortName}` : undefined}
    >
      <span className="flex size-10 shrink-0 items-center justify-center rounded bg-black/25 font-title text-[17px] font-bold">
        {initials(club.shortName)}
      </span>
      <div className={`min-w-0 flex-1 ${side === 'away' ? 'text-right' : ''}`}>
        <div className="truncate font-title text-[28px] font-bold uppercase ">{club.shortName}</div>
        <div className="truncate text-[11px] font-bold font-mono uppercase tracking-wider opacity-80">{detail}</div>
      </div>
      {/* O número muda com o golo e dá um pulo (o 0 do início, não) */}
      <span
        key={goals ?? '-'}
        className={`flex h-12 min-w-[54px] shrink-0 items-center justify-center rounded bg-[#0d0e13]/90 px-2 font-title text-[34px] font-bold leading-none text-white shadow-[inset_0_2px_4px_rgba(0,0,0,0.8)] ${goals ? 'cm-score-pop' : ''}`}
      >
        {goals ?? ''}
      </span>
    </div>
  )
}
