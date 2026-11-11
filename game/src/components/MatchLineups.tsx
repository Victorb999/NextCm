import type { ClubRef } from '../bindings/ClubRef'
import type { LineupEntry } from '../matchLineup'
import { useNav } from '../nav'
import { Link } from './Screen'

// As duas listas de uma partida lado a lado, como as "Classificações Jogadores" do CM:
// número, cartão, nome (ligação ao perfil), "sub"/"ent" com o minuto, nota e golos.

const fullName = (listName: string) => listName.split(', ').reverse().join(' ')

function Side({ club, entries, motm }: { club: ClubRef; entries: LineupEntry[]; motm: number | null }) {
  const nav = useNav()
  return (
    <div className="min-w-0 flex-1">
      <div
        className="mb-1 flex h-8 items-center justify-center rounded font-title text-[18px] font-bold"
        style={{ color: club.fg ?? '#fff', background: club.bg ?? '#333' }}
      >
        {club.shortName}
      </div>
      <div className="cm-zebra">
        {entries.map((p) => (
          <div key={p.id} className={`flex h-[23px] items-center gap-1.5 px-1 text-[13.5px] ${p.id === motm ? 'ring-1 ring-cm-yellow' : ''}`}>
            <span
              className={`w-7 shrink-0 rounded-sm text-center font-mono text-[12px] leading-[19px] ${p.played ? 'bg-[#262626] text-white' : 'bg-[#161660] text-cm-lilac'}`}
            >
              {p.number ?? ''}
            </span>
            <span className="flex w-2.5 shrink-0 justify-center">
              {p.red ? <span className="h-3 w-2 bg-cm-red" /> : p.yellow ? <span className="h-3 w-2 bg-cm-yellow" /> : null}
            </span>
            {/* Quem saiu fica num lilás claro, perto do roxo dos suplentes que não jogaram */}
            <span
              className={`min-w-0 flex-1 truncate ${
                p.id === motm ? 'font-bold text-cm-yellow' : !p.played ? 'text-cm-purplelight' : p.off !== null ? 'text-cm-lilac' : 'text-white'
              }`}
            >
              <Link onClick={() => nav.go({ kind: 'person', id: p.id })}>{fullName(p.name)}</Link>
            </span>
            <span className="w-14 shrink-0 text-right text-[12px]">
              {p.off !== null ? <span className="text-cm-muted">sub {p.off}</span> : p.on !== null ? <span className="text-cm-orange">ent {p.on}</span> : null}
            </span>
            <span className="w-6 shrink-0 text-right font-bold text-cm-magenta">{p.played ? (p.rating ?? '-') : ''}</span>
            <span className="w-4 shrink-0 text-right font-bold text-cm-yellow">{p.goals || ''}</span>
          </div>
        ))}
      </div>
    </div>
  )
}

export default function MatchLineups({
  home,
  away,
  motm = null,
}: {
  home: { club: ClubRef; entries: LineupEntry[] }
  away: { club: ClubRef; entries: LineupEntry[] }
  motm?: number | null
}) {
  return (
    <div className="flex gap-4 p-3">
      <Side club={home.club} entries={home.entries} motm={motm} />
      <Side club={away.club} entries={away.entries} motm={motm} />
    </div>
  )
}
