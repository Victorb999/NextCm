import { useEffect, useState } from 'react'
import { api } from '../../api'
import type { CompetitionStats } from '../../bindings/CompetitionStats'
import type { PlayerStatRow } from '../../bindings/PlayerStatRow'
import { Empty, Link, Panel, PurpleMenu } from '../../components/Screen'
import { shortListName, xgText } from '../../format'
import { useNav } from '../../nav'

// A aba Estatísticas de uma competição (marco 4.3): os jogadores por um critério (golos,
// assistências, nota, golos esperados...) e as equipas com os golos e os golos esperados.

type Criterion = 'goals' | 'assists' | 'rating' | 'xg' | 'overXg' | 'shots' | 'mom'

const CRITERIA: { key: Criterion; label: string }[] = [
  { key: 'goals', label: 'Golos' },
  { key: 'assists', label: 'Assistências' },
  { key: 'rating', label: 'Nota Média' },
  { key: 'xg', label: 'Golos Esperados' },
  { key: 'overXg', label: 'Golos acima do Esperado' },
  { key: 'shots', label: 'Remates' },
  { key: 'mom', label: 'Homem do Jogo' },
]

const VALUE: Record<Criterion, (p: PlayerStatRow) => number> = {
  goals: (p) => p.goals,
  assists: (p) => p.assists,
  rating: (p) => p.rating,
  xg: (p) => p.xg,
  overXg: (p) => p.goals - p.xg,
  shots: (p) => p.shots,
  mom: (p) => p.mom,
}

/** A coluna que se destaca para cada critério (J, Gol, Ast, Nota, Rem, xG, G-xG, HJ). */
const COL: Record<Criterion, number> = { goals: 1, assists: 2, rating: 3, shots: 4, xg: 5, overXg: 6, mom: 7 }

/** Jogadores mostrados na lista. */
const SHOWN = 50
/** Na Nota Média, só quem jogou pelo menos tantos jogos. */
const RATING_APPS = 3

const TH = 'bg-cm-headdeep px-2 text-[11px] font-bold uppercase tracking-[0.1em] text-cm-yellow'

const signed = (v: number) => `${v > 0 ? '+' : ''}${xgText(v)}`

export default function StatsPanel({ id, date, own }: { id: number; date?: string; own?: number }) {
  const nav = useNav()
  const [stats, setStats] = useState<CompetitionStats | null>(null)
  const [criterion, setCriterion] = useState<Criterion>('goals')
  useEffect(() => {
    if (!date) return
    api.competitionStats(id).then(setStats, () => setStats(null))
  }, [id, date])
  if (!stats) return null
  const value = VALUE[criterion]
  const players = stats.players
    .filter((p) => criterion !== 'rating' || p.apps >= RATING_APPS)
    .sort((a, b) => value(b) - value(a) || b.apps - a.apps || a.person - b.person)
    .slice(0, SHOWN)
  const club = (c: PlayerStatRow['club']) => (
    <Link
      onClick={() => nav.go({ kind: 'club', id: c.id })}
      className={`truncate ${c.id === own ? 'font-bold text-cm-pink' : 'text-cm-magenta'}`}
    >
      {c.shortName}
    </Link>
  )
  const cols = ['J', 'Gol', 'Ast', 'Nota', 'Rem', 'xG', 'G-xG', 'HJ']
  return (
    <div className="flex min-h-0 flex-1 gap-2">
      <Panel
        title={`Jogadores · ${stats.season}`}
        right={<PurpleMenu label="Critério" align="right" options={CRITERIA} value={criterion} onChange={setCriterion} />}
        className="min-w-0 flex-1"
      >
        {players.length === 0 ? (
          <Empty>Sem estatísticas de jogadores: os jogos desta competição só têm o resultado.</Empty>
        ) : (
          <div className="min-h-0 flex-1 overflow-auto">
            <table className="w-full border-collapse">
              <thead className="sticky top-0 z-10">
                <tr className="h-8">
                  <th className={`${TH} w-10 text-center`}>#</th>
                  <th className={`${TH} text-left`}>Jogador</th>
                  <th className={`${TH} w-36 text-left`}>Clube</th>
                  {cols.map((c) => (
                    <th key={c} className={`${TH} w-14 text-center`}>
                      {c}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody className="cm-zebra">
                {players.map((p, i) => (
                  <tr key={p.person} className="h-[26px] text-[14px]">
                    <td className="text-center tabular-nums text-cm-dim">{i + 1}</td>
                    <td className="max-w-0 truncate px-2">
                      <Link onClick={() => nav.go({ kind: 'person', id: p.person })} className="font-title text-[15px] text-white">
                        {shortListName(p.name)}
                      </Link>
                    </td>
                    <td className="max-w-0 truncate px-2">{club(p.club)}</td>
                    {[
                      p.apps,
                      p.goals,
                      p.assists,
                      p.rating.toFixed(2).replace('.', ','),
                      p.shots,
                      xgText(p.xg),
                      signed(p.goals - p.xg),
                      p.mom,
                    ].map((v, k) => (
                      <td
                        key={k}
                        className={`text-center tabular-nums ${k === COL[criterion] ? 'font-bold text-cm-yellow' : 'text-cm-text'}`}
                      >
                        {v}
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </Panel>
      <Panel title="Equipas" className="w-[440px] shrink-0">
        <div className="min-h-0 flex-1 overflow-auto">
          <table className="w-full border-collapse">
            <thead className="sticky top-0 z-10">
              <tr className="h-8">
                <th className={`${TH} text-left`}>Clube</th>
                {['J', 'M', 'xG', 'S', 'xGS'].map((c) => (
                  <th key={c} className={`${TH} w-14 text-center`}>
                    {c}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody className="cm-zebra">
              {stats.teams.map((t) => (
                <tr key={t.club.id} className="h-[26px] text-[14px]">
                  <td className="max-w-0 truncate px-2">{club(t.club)}</td>
                  <td className="text-center tabular-nums text-cm-text">{t.games}</td>
                  <td className="text-center tabular-nums text-cm-text">{t.goalsFor}</td>
                  <td className="text-center font-bold tabular-nums text-cm-yellow">{xgText(t.xgFor)}</td>
                  <td className="text-center tabular-nums text-cm-text">{t.goalsAgainst}</td>
                  <td className="text-center tabular-nums text-cm-orange">{xgText(t.xgAgainst)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Panel>
    </div>
  )
}
