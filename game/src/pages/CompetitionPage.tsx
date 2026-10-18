import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { api } from '../api'
import type { ClubRef } from '../bindings/ClubRef'
import type { CompetitionProfile } from '../bindings/CompetitionProfile'
import type { Fixture } from '../bindings/Fixture'
import type { StandingRow } from '../bindings/StandingRow'
import { FixtureRows } from '../components/Fixtures'
import { BottomBar, Link, Tabs, TitleBand } from '../components/Screen'
import { continentPt } from '../continents'
import { matchDate } from '../format'
import { useGame } from '../game'
import { useEscBack, useNav } from '../nav'

const TABS = [
  { key: 'table', label: 'Classificação' },
  { key: 'fixtures', label: 'Calendário' },
  { key: 'results', label: 'Resultados' },
  { key: 'stats', label: 'Estatísticas', disabled: true },
  { key: 'history', label: 'Historial' },
]

function ClubLink({ club }: { club: ClubRef | null }) {
  const nav = useNav()
  if (!club) return <span className="text-[#666]">-</span>
  return (
    <Link onClick={() => nav.go({ kind: 'club', id: club.id })} className="text-cm-magenta">
      {club.shortName}
    </Link>
  )
}

function History({ c }: { c: CompetitionProfile }) {
  if (c.history.length === 0) return <div className="p-6 text-center text-cm-magenta">Sem historial registado.</div>
  const head = ['Ano', 'Campeão', 'Vice', '3º']
  return (
    <div className="min-h-0 flex-1 overflow-auto px-1.5 pt-2">
      <table className="w-full border-collapse">
        <thead className="sticky top-0">
          <tr className="h-6">
            {head.map((h) => (
              <th key={h} className="border border-black bg-cm-head px-2 text-left font-normal text-white">
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {c.history.map((h, i) => (
            <tr key={i} className="h-[24px] border-b border-cm-line bg-black/40 text-[15px]">
              <td className="w-20 px-2 tabular-nums text-cm-yellow">{h.year ?? '-'}</td>
              <td className="px-2">
                <ClubLink club={h.winner} />
              </td>
              <td className="px-2">
                <ClubLink club={h.runnerUp} />
              </td>
              <td className="px-2">
                <ClubLink club={h.third} />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

/** Classificação: a real quando a competição está viva no jogo; senão, a de pré-temporada
 * (todos a zero, por ordem alfabética), como no CM. */
function Table({ c, rows, own }: { c: CompetitionProfile; rows: StandingRow[]; own?: number }) {
  const nav = useNav()
  const shown: { club: ClubRef; values: number[] }[] = rows.length
    ? rows.map((r) => ({ club: r.club, values: [r.played, r.won, r.drawn, r.lost, r.goalsFor, r.goalsAgainst, r.points] }))
    : [...c.clubs]
        .sort((a, b) => a.club.shortName.localeCompare(b.club.shortName))
        .map((r) => ({ club: r.club, values: [0, 0, 0, 0, 0, 0, 0] }))
  const cols = ['J', 'V', 'E', 'D', 'M', 'S', 'P']
  return (
    <div className="min-h-0 flex-1 overflow-auto px-3 pb-2">
      <div className="py-1.5 text-center font-title text-xl font-bold text-cm-yellow">Classificação Geral</div>
      <table className="w-full border-separate border-spacing-x-1 border-spacing-y-0">
        <thead className="sticky top-0 bg-cm-panel">
          <tr className="h-6">
            <th className="w-12" />
            <th />
            {cols.map((h) => (
              <th key={h} className="w-14 border border-black bg-cm-head font-normal text-white">
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {shown.map((row, i) => (
            <tr key={row.club.id} className="h-[23px]">
              <td className="border border-cm-tabline bg-black/60 text-center text-white">{i + 1}º</td>
              <td className="px-3">
                <Link
                  onClick={() => nav.go({ kind: 'club', id: row.club.id })}
                  className={`font-title text-[17px] ${row.club.id === own ? 'text-cm-pink' : 'text-white'}`}
                >
                  {row.club.shortName}
                </Link>
              </td>
              {row.values.map((v, j) => (
                <td
                  key={cols[j]}
                  className={`text-center tabular-nums text-white ${j === 6 ? 'border border-cm-tabline bg-black/60' : ''}`}
                >
                  {v}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

/** Jornadas agrupadas: número da jornada → jogos. */
function byRound(fixtures: Fixture[]): Map<number, Fixture[]> {
  const rounds = new Map<number, Fixture[]>()
  for (const f of fixtures) rounds.set(f.round, [...(rounds.get(f.round) ?? []), f])
  return rounds
}

/** Calendário: uma jornada de cada vez, com ◀ ▶; abre na próxima por jogar. */
function Calendar({ fixtures, own }: { fixtures: Fixture[]; own?: number }) {
  const rounds = byRound(fixtures)
  const last = Math.max(...rounds.keys())
  const next = fixtures.find((f) => f.homeGoals === null)?.round ?? last
  const [round, setRound] = useState(next)
  const games = rounds.get(round) ?? []
  return (
    <div className="flex min-h-0 flex-1 flex-col px-3 pb-2">
      <div className="flex shrink-0 items-center justify-center gap-2 py-1.5">
        <button className="cm-purple h-6 w-10" disabled={round <= 1} onClick={() => setRound(round - 1)} aria-label="Jornada anterior">
          ◀
        </button>
        <div className="w-72 text-center font-title text-xl font-bold text-cm-yellow">
          {round}ª jornada{games[0] ? ` · ${matchDate(games[0].date).slice(4)}` : ''}
        </div>
        <button className="cm-purple h-6 w-10" disabled={round >= last} onClick={() => setRound(round + 1)} aria-label="Jornada seguinte">
          ▶
        </button>
      </div>
      <div className="min-h-0 flex-1 overflow-auto">
        <FixtureRows fixtures={games} own={own} />
      </div>
    </div>
  )
}

/** Resultados: jornadas já disputadas, da mais recente para a mais antiga. */
function Results({ fixtures, own }: { fixtures: Fixture[]; own?: number }) {
  const rounds = [...byRound(fixtures.filter((f) => f.homeGoals !== null)).entries()].reverse()
  if (rounds.length === 0) return <div className="p-6 text-center text-cm-magenta">Ainda não há resultados.</div>
  return (
    <div className="min-h-0 flex-1 overflow-auto px-3 pb-2">
      {rounds.map(([round, games]) => (
        <div key={round}>
          <div className="pb-1 pt-2 text-center font-title text-lg font-bold text-cm-yellow">{round}ª jornada</div>
          <FixtureRows fixtures={games} own={own} />
        </div>
      ))}
    </div>
  )
}

/** Competição: abre na Classificação (ou na aba pedida, ex.: Resultados depois de um jogo). */
export default function CompetitionPage({ id, initialTab }: { id: number; initialTab?: string }) {
  const nav = useNav()
  const game = useGame()
  useEscBack()
  const [comp, setComp] = useState<CompetitionProfile | null>(null)
  const [table, setTable] = useState<StandingRow[]>([])
  const [fixtures, setFixtures] = useState<Fixture[]>([])
  const [error, setError] = useState<string | null>(null)
  const [tab, setTab] = useState(initialTab ?? 'table')

  useEffect(() => {
    api.getCompetition(id).then(
      (c) => {
        setComp(c)
        // Sem participantes (ex.: Libertadores fora do jogo): só há o historial
        if (!initialTab && c.clubs.length === 0 && c.history.length > 0) setTab('history')
      },
      (e) => setError(String(e)),
    )
  }, [id, initialTab])

  // Classificação e calendário reais (só nas competições vivas do jogo); recarrega a cada dia
  const date = game.info?.date
  useEffect(() => {
    if (!date) return
    api.leagueTable(id).then(setTable, () => {})
    api.listFixtures(id).then(setFixtures, () => {})
  }, [id, date])

  const own = game.info?.club.id
  const where: ReactNode = comp?.nation ? (
    <Link onClick={() => nav.go({ kind: 'nation', id: comp.nationId! })}>{comp.nation}</Link>
  ) : (
    continentPt(comp?.continent ?? 'Mundial')
  )

  return (
    <div className="flex h-full flex-col">
      <TitleBand title={comp?.name ?? ''} fg="#ffffff" bg="#1e9cf0" />
      <Tabs
        tabs={TABS.map((t) => {
          if (t.key === 'table') return { ...t, disabled: !comp || (comp.clubs.length === 0 && table.length === 0) }
          if (t.key === 'fixtures') return { ...t, disabled: fixtures.length === 0 }
          if (t.key === 'results') return { ...t, disabled: !fixtures.some((f) => f.homeGoals !== null) }
          return t
        })}
        active={tab}
        onChange={setTab}
      />
      <div className="mx-1.5 mt-1.5 flex min-h-0 flex-1 flex-col border border-cm-line bg-cm-panel/80">
        {error && <div className="bg-red-900 px-2 py-1 text-white">{error}</div>}
        {comp && (
          <div className="shrink-0 pt-1.5 text-center text-cm-magenta">
            {where} · Reputação {comp.reputation}
            {comp.kind === 'national' && ' · Seleções'}
          </div>
        )}
        {comp && tab === 'table' && <Table c={comp} rows={table} own={own} />}
        {tab === 'fixtures' && fixtures.length > 0 && <Calendar fixtures={fixtures} own={own} />}
        {tab === 'results' && <Results fixtures={fixtures} own={own} />}
        {comp && tab === 'history' && <History c={comp} />}
      </div>
      <BottomBar onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />
    </div>
  )
}
