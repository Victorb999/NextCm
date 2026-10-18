import type { ClubRef } from '../bindings/ClubRef'
import type { Fixture } from '../bindings/Fixture'
import type { StandingRow } from '../bindings/StandingRow'
import { matchDate } from '../format'
import { useNav } from '../nav'
import { Link } from './Screen'

// Listas de jogos e o painel Próximo/Último Jogo, no estilo das telas do CM.

const played = (f: Fixture) => f.homeGoals !== null && f.awayGoals !== null

function ClubName({ club, own, className = '' }: { club: ClubRef; own?: number; className?: string }) {
  const nav = useNav()
  return (
    <Link
      onClick={() => nav.go({ kind: 'club', id: club.id })}
      className={`font-title text-[17px] ${club.id === own ? 'text-cm-pink' : 'text-white'} ${className}`}
    >
      {club.shortName}
    </Link>
  )
}

/** Jogos de uma jornada ou de um dia: casa  2 - 1  fora. */
export function FixtureRows({ fixtures, own, showDate = true }: { fixtures: Fixture[]; own?: number; showDate?: boolean }) {
  return (
    <table className="w-full border-collapse">
      <tbody>
        {fixtures.map((f) => (
          <tr key={f.id} className="h-[25px] border-b border-cm-line bg-black/40 text-[15px]">
            {showDate && <td className="w-36 whitespace-nowrap px-2 text-cm-magenta">{matchDate(f.date)}</td>}
            <td className="px-2 text-right">
              <ClubName club={f.home} own={own} />
            </td>
            <td className="w-20 whitespace-nowrap text-center font-bold tabular-nums text-cm-yellow">
              {played(f) ? `${f.homeGoals} - ${f.awayGoals}` : 'v'}
            </td>
            <td className="px-2">
              <ClubName club={f.away} own={own} />
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  )
}

/** Calendário de um clube: data, competição, adversário (C/F) e resultado. */
export function ClubFixtureList({ clubId, fixtures }: { clubId: number; fixtures: Fixture[] }) {
  if (fixtures.length === 0) return <div className="p-6 text-center text-cm-magenta">Sem jogos marcados.</div>
  return (
    <div className="min-h-0 flex-1 overflow-auto px-1.5 py-2">
      <table className="w-full border-collapse">
        <thead className="sticky top-0">
          <tr className="h-6">
            {['Data', 'Competição', '', 'Adversário', 'Resultado'].map((h, i) => (
              <th key={i} className="border border-black bg-cm-head px-2 text-left font-normal text-white">
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {fixtures.map((f) => {
            const home = f.home.id === clubId
            const opponent = home ? f.away : f.home
            const [gf, ga] = home ? [f.homeGoals, f.awayGoals] : [f.awayGoals, f.homeGoals]
            const outcome = gf === null || ga === null ? null : gf > ga ? 'V' : gf === ga ? 'E' : 'D'
            const colour = outcome === 'V' ? 'text-cm-green' : outcome === 'D' ? 'text-[#ff4040]' : 'text-cm-yellow'
            return (
              <tr key={f.id} className="h-[24px] border-b border-cm-line bg-black/40 text-[15px]">
                <td className="w-36 whitespace-nowrap px-2 text-cm-magenta">{matchDate(f.date)}</td>
                <td className="w-40 px-2 text-white">
                  {f.competition} <span className="text-[#aab3c4]">J{f.round}</span>
                </td>
                <td className="w-8 text-center text-cm-orange">{home ? 'C' : 'F'}</td>
                <td className="px-2">
                  <ClubName club={opponent} />
                </td>
                <td className={`w-28 px-2 font-bold tabular-nums ${colour}`}>
                  {outcome ? `${outcome} ${gf}-${ga}` : '-'}
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </div>
  )
}

function Side({ club, row, align }: { club: ClubRef; row?: StandingRow; align: 'left' | 'right' }) {
  const colours = club.fg && club.bg ? { color: club.fg, background: club.bg } : { color: '#fff', background: '#3a3a3a' }
  return (
    <div className={`flex flex-1 flex-col gap-1 ${align === 'right' ? 'items-end' : 'items-start'}`}>
      <div className="w-full border-2 border-black px-3 py-2 text-center font-title text-[26px] font-bold" style={colours}>
        <ClubName club={club} className="!text-inherit !text-[26px]" />
      </div>
      {row && (
        <div className="w-full bg-black/50 px-3 py-1 text-center text-[15px] text-white">
          {row.played > 0 ? (
            <>
              <span className="text-cm-yellow">{row.position}º</span> · {row.points} pts · J {row.played} · {row.goalsFor}-{row.goalsAgainst}
            </>
          ) : (
            'Ainda sem jogos'
          )}
          {row.form && (
            <span className="ml-2 tracking-widest">
              {[...row.form].map((c, i) => (
                <span key={i} className={c === 'V' ? 'text-cm-green' : c === 'D' ? 'text-[#ff4040]' : 'text-cm-yellow'}>
                  {c}
                </span>
              ))}
            </span>
          )}
        </div>
      )}
    </div>
  )
}

/** Próximo Jogo / Último Jogo: os dois clubes com as cores, o resultado e a situação na tabela. */
export function MatchPanel({ fixture, table, empty }: { fixture: Fixture | undefined; table: StandingRow[]; empty: string }) {
  if (!fixture) return <div className="p-6 text-center text-cm-magenta">{empty}</div>
  const row = (id: number) => table.find((r) => r.club.id === id)
  return (
    <div className="mx-auto mt-4 flex w-full max-w-3xl flex-col gap-3 px-3">
      <div className="text-center font-title text-xl font-bold text-cm-yellow">
        {fixture.competition} · {fixture.round}ª jornada
      </div>
      <div className="text-center text-cm-magenta">{matchDate(fixture.date)}</div>
      <div className="flex items-start gap-3">
        <Side club={fixture.home} row={row(fixture.home.id)} align="right" />
        <div className="flex h-[52px] w-24 shrink-0 items-center justify-center border-2 border-black bg-black font-title text-[28px] font-bold tabular-nums text-cm-yellow">
          {played(fixture) ? `${fixture.homeGoals}-${fixture.awayGoals}` : 'v'}
        </div>
        <Side club={fixture.away} row={row(fixture.away.id)} align="left" />
      </div>
    </div>
  )
}
