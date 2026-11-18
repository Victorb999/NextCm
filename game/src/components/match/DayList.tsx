import { Trophy } from 'lucide-react'
import type { ClubRef } from '../../bindings/ClubRef'
import type { Fixture } from '../../bindings/Fixture'
import type { StandingRow } from '../../bindings/StandingRow'
import { pensText } from '../../format'
import { useNav } from '../../nav'
import { BottomBar, Link, Panel, ScreenHeader } from '../Screen'

/** Faixa azul com o nome da competição, como nas telas de jogos do dia do CM. */
/** Jogos do dia na competição, como "Jogos da Tarde" / "Resultados da Tarde" do CM. */
export function DayList({
  title,
  fixture,
  games,
  table,
  own,
  scores,
  onNext,
}: {
  title: string
  fixture: Fixture
  games: Fixture[]
  table: StandingRow[]
  own: number
  scores: boolean
  onNext: () => void
}) {
  const pos = (id: number) => {
    const r = table.find((r) => r.club.id === id)
    return r && r.played > 0 ? `${r.position}º` : ''
  }
  const nav = useNav()
  // A casa roxa ao lado de cada clube: a posição na liga ou, no mata-mata, a divisão (copas
  // nacionais) ou o país (internacionais), que o core manda
  const chip = 'cm-chip min-w-11 shrink-0 whitespace-nowrap px-1.5 text-center font-mono text-[12px] font-bold leading-[22px]'
  // Os clubes são ligações; com resultado, o placar abre a ficha da partida (esconde o dia de jogo, como qualquer navegação)
  const name = (c: ClubRef, align: string) => (
    <span className={`flex-1 truncate text-[17px] font-semibold ${align} ${c.id === own ? 'text-cm-yellow' : 'text-white'}`}>
      <Link onClick={() => nav.go({ kind: 'club', id: c.id })}>{c.shortName}</Link>
    </span>
  )
  return (
    <div className="cm-backdrop absolute inset-0 z-50 flex flex-col">
      <ScreenHeader
        icon={
          <span className="flex size-11 items-center justify-center rounded bg-[#1e9cf0] text-white">
            <Trophy size={22} />
          </span>
        }
        eyebrow={<Link onClick={() => nav.go({ kind: 'competition', id: fixture.competitionId })}>{fixture.competition}</Link>}
        title={`${title} · ${fixture.label}`}
      />
      <Panel title={`${games.length} jogos`} className="mx-2 min-h-0 flex-1">
        <div className="cm-zebra min-h-0 flex-1 overflow-auto">
          {games.map((f) => (
            <div key={f.id} className="flex h-[32px] items-center gap-3 px-4">
              <span className={chip}>{f.knockout ? (f.homeTag ?? '') : pos(f.home.id)}</span>
              {name(f.home, 'text-right')}
              {scores && f.homeGoals !== null ? (
                <button
                  onClick={() => nav.go({ kind: 'match', id: f.id })}
                  title="Ver a ficha da partida"
                  className={`w-16 shrink-0 rounded bg-black/40 text-center font-mono text-[17px] font-bold text-cm-magenta hover:bg-cm-sel hover:text-white ${pensText(f) ? 'leading-[18px]' : 'leading-[26px]'}`}
                >
                  {f.homeGoals}:{f.awayGoals}
                  {/* Os pênaltis ou a prorrogação do mata-mata, por baixo do placar */}
                  {pensText(f) && (
                    <span className="block pb-0.5 font-sans text-[10px] font-normal leading-none text-cm-orange">
                      {pensText(f).trim().slice(1, -1)}
                    </span>
                  )}
                </button>
              ) : (
                <span className="w-16 shrink-0 rounded bg-black/40 text-center font-mono text-[17px] font-bold leading-[26px] text-cm-magenta">
                  v
                </span>
              )}
              {name(f.away, 'text-left')}
              <span className={chip}>{f.knockout ? (f.awayTag ?? '') : pos(f.away.id)}</span>
            </div>
          ))}
        </div>
      </Panel>
      <BottomBar onRight={onNext} />
    </div>
  )
}

// ---------------------------------------------------------------------------------------
// A partida
