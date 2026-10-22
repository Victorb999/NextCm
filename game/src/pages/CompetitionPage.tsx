import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { api } from '../api'
import type { ClubRef } from '../bindings/ClubRef'
import type { CompetitionProfile } from '../bindings/CompetitionProfile'
import type { Fixture } from '../bindings/Fixture'
import type { StageView } from '../bindings/StageView'
import type { StandingRow } from '../bindings/StandingRow'
import type { TieView } from '../bindings/TieView'
import { FixtureRows, RoundGames } from '../components/Fixtures'
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

const COLS = ['J', 'V', 'E', 'D', 'M', 'S', 'P']
const values = (r: StandingRow) => [r.played, r.won, r.drawn, r.lost, r.goalsFor, r.goalsAgainst, r.points]

/** Classificação: a real quando a competição está viva no jogo; senão, a de pré-temporada
 * (todos a zero, por ordem alfabética), como no CM. */
function Table({ c, rows, own, title = 'Classificação Geral' }: { c: CompetitionProfile; rows: StandingRow[]; own?: number; title?: string }) {
  const nav = useNav()
  const shown: { club: ClubRef; values: number[] }[] = rows.length
    ? rows.map((r) => ({ club: r.club, values: values(r) }))
    : [...c.clubs]
        .sort((a, b) => a.club.shortName.localeCompare(b.club.shortName))
        .map((r) => ({ club: r.club, values: [0, 0, 0, 0, 0, 0, 0] }))
  return (
    <div className="min-h-0 flex-1 overflow-auto px-3 pb-2">
      <div className="py-1.5 text-center font-title text-xl font-bold text-cm-yellow">{title}</div>
      <table className="w-full border-separate border-spacing-x-1 border-spacing-y-0">
        <thead className="sticky top-0 bg-cm-panel">
          <tr className="h-6">
            <th className="w-12" />
            <th />
            {COLS.map((h) => (
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
                  key={COLS[j]}
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

/** Um grupo em ponto pequeno, para caberem dois por linha. */
function GroupBox({ name, rows, own }: { name: string; rows: StandingRow[]; own?: number }) {
  const nav = useNav()
  return (
    <div>
      <div className="pb-0.5 text-center font-title text-lg font-bold text-cm-yellow">{name}</div>
      <table className="w-full border-separate border-spacing-x-0.5 border-spacing-y-0 text-[14px]">
        <thead>
          <tr className="h-5">
            <th />
            {COLS.map((h) => (
              <th key={h} className="w-8 border border-black bg-cm-head font-normal text-white">
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.club.id} className="h-[21px]">
              <td className="truncate px-1">
                <Link
                  onClick={() => nav.go({ kind: 'club', id: r.club.id })}
                  className={`font-title text-[16px] ${r.club.id === own ? 'text-cm-pink' : 'text-white'}`}
                >
                  {r.position}. {r.club.shortName}
                </Link>
              </td>
              {values(r).map((v, j) => (
                <td key={COLS[j]} className={`text-center tabular-nums text-white ${j === 6 ? 'bg-black/60' : ''}`}>
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

/** Uma eliminatória: os dois clubes, os resultados de cada mão, o total e os penáltis. */
function Tie({ tie, own }: { tie: TieView; own?: number }) {
  const nav = useNav()
  const name = (c: ClubRef, align: string) => (
    <Link
      onClick={() => nav.go({ kind: 'club', id: c.id })}
      className={`block truncate font-title text-[18px] ${align} ${tie.winner === c.id ? 'font-bold' : ''} ${
        c.id === own ? 'text-cm-pink' : tie.winner !== null && tie.winner !== c.id ? 'text-[#9aa0aa]' : 'text-white'
      }`}
    >
      {c.shortName}
    </Link>
  )
  // Golos de cada mão do ponto de vista do primeiro clube
  const legs = tie.games.map((g) =>
    g.homeGoals === null ? 'v' : g.home.id === tie.first.id ? `${g.homeGoals}-${g.awayGoals}` : `${g.awayGoals}-${g.homeGoals}`,
  )
  const extra = [
    tie.aggregate ? `total ${tie.aggregate[0]}-${tie.aggregate[1]}` : '',
    tie.pens ? `${tie.pens[0]}-${tie.pens[1]} g.p.` : '',
  ]
    .filter(Boolean)
    .join(', ')
  return (
    <div className="flex h-[26px] items-center gap-2 border-b border-cm-line bg-black/40 px-2">
      <div className="min-w-0 flex-1">{name(tie.first, 'text-right')}</div>
      <span className="w-28 shrink-0 text-center font-bold tabular-nums text-cm-yellow">{legs.join(' · ')}</span>
      <div className="min-w-0 flex-1">{name(tie.second, 'text-left')}</div>
      <span className="w-36 shrink-0 text-right text-[13px] text-cm-orange">{extra}</span>
    </div>
  )
}

/** Classificação de uma competição viva, fase a fase (pontos corridos, grupos, eliminatórias). */
function Stages({ c, stages, own }: { c: CompetitionProfile; stages: StageView[]; own?: number }) {
  const [index, setIndex] = useState(stages.length - 1)
  const stage = stages[Math.min(index, stages.length - 1)]
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {stages.length > 1 && (
        <div className="flex shrink-0 justify-center gap-1 pt-1.5">
          {stages.map((st, i) => (
            <button key={st.name} className={`cm-purple h-6 w-44 ${i === index ? '!text-cm-yellow' : ''}`} onClick={() => setIndex(i)}>
              {st.name}
            </button>
          ))}
        </div>
      )}
      {stage.kind === 'league' && (
        <Table c={c} rows={stage.tables[0]?.rows ?? []} own={own} title={stages.length > 1 ? stage.name : 'Classificação Geral'} />
      )}
      {stage.kind === 'groups' && (
        <div className="grid min-h-0 flex-1 grid-cols-2 content-start gap-x-6 gap-y-2 overflow-auto px-3 py-2">
          {stage.tables.map((t) => (
            <GroupBox key={t.name} name={t.name} rows={t.rows} own={own} />
          ))}
        </div>
      )}
      {stage.kind === 'knockout' && (
        <div className="min-h-0 flex-1 overflow-auto px-3 pb-2">
          {[...stage.rounds].reverse().map((r) => (
            <div key={r.name}>
              <div className="pb-1 pt-2 text-center font-title text-lg font-bold text-cm-yellow">{r.name}</div>
              {r.ties.map((t, i) => (
                <Tie key={i} tie={t} own={own} />
              ))}
            </div>
          ))}
        </div>
      )}
    </div>
  )
}

/** "Jornada" de um jogo para o Calendário: a etiqueta sem o grupo ("Grupo B · 3ª jornada" → "3ª jornada"). */
const roundKey = (f: Fixture) => f.label.replace(/^Grupo [A-Z] · /, '')

/** Jogos agrupados por jornada (ou ronda e mão), pela ordem das datas. */
function byRound(fixtures: Fixture[]): [string, Fixture[]][] {
  const rounds = new Map<string, Fixture[]>()
  const sorted = [...fixtures].sort((a, b) => a.date.localeCompare(b.date) || a.id - b.id)
  for (const f of sorted) rounds.set(roundKey(f), [...(rounds.get(roundKey(f)) ?? []), f])
  return [...rounds.entries()]
}

/** Calendário: uma jornada de cada vez, com ◀ ▶; abre na próxima por jogar. */
function Calendar({ fixtures, table, own }: { fixtures: Fixture[]; table: StandingRow[]; own?: number }) {
  const rounds = byRound(fixtures)
  const next = rounds.findIndex(([, games]) => games.some((f) => f.homeGoals === null))
  const [index, setIndex] = useState(next < 0 ? rounds.length - 1 : next)
  const [name, games] = rounds[Math.min(index, rounds.length - 1)]
  return (
    <div className="flex min-h-0 flex-1 flex-col px-3 pb-2">
      <div className="flex shrink-0 items-center justify-center gap-2 py-1.5">
        <button className="cm-purple h-6 w-10" disabled={index <= 0} onClick={() => setIndex(index - 1)} aria-label="Jornada anterior">
          ◀
        </button>
        <div className="w-[26rem] text-center font-title text-xl font-bold text-cm-yellow">
          {name}
          {games[0] ? ` · ${matchDate(games[0].date).slice(4)}` : ''}
        </div>
        <button
          className="cm-purple h-6 w-10"
          disabled={index >= rounds.length - 1}
          onClick={() => setIndex(index + 1)}
          aria-label="Jornada seguinte"
        >
          ▶
        </button>
      </div>
      <div className="min-h-0 flex-1 overflow-auto pt-1">
        <RoundGames fixtures={games} table={table} own={own} />
      </div>
    </div>
  )
}

/** Resultados: jornadas já disputadas, da mais recente para a mais antiga. */
function Results({ fixtures, own }: { fixtures: Fixture[]; own?: number }) {
  const rounds = byRound(fixtures.filter((f) => f.homeGoals !== null)).reverse()
  if (rounds.length === 0) return <div className="p-6 text-center text-cm-magenta">Ainda não há resultados.</div>
  return (
    <div className="min-h-0 flex-1 overflow-auto px-3 pb-2">
      {rounds.map(([name, games]) => (
        <div key={name}>
          <div className="pb-1 pt-2 text-center font-title text-lg font-bold text-cm-yellow">{name}</div>
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
  const [stages, setStages] = useState<StageView[]>([])
  const [fixtures, setFixtures] = useState<Fixture[]>([])
  const [error, setError] = useState<string | null>(null)
  const [tab, setTab] = useState(initialTab ?? 'table')

  useEffect(() => {
    api.getCompetition(id).then(
      (c) => {
        setComp(c)
        // Sem participantes no banco (ex.: Libertadores): o historial, até haver jogos
        if (!initialTab && c.clubs.length === 0 && c.history.length > 0) setTab((t) => (t === 'table' ? 'history' : t))
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
    api.competitionStages(id).then(
      (st) => {
        setStages(st)
        if (st.length > 0 && !initialTab) setTab((t) => (t === 'history' ? 'table' : t))
      },
      () => {},
    )
  }, [id, date, initialTab])

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
          if (t.key === 'table') return { ...t, disabled: !comp || (comp.clubs.length === 0 && stages.length === 0) }
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
        {comp && tab === 'table' && (stages.length > 0 ? <Stages c={comp} stages={stages} own={own} /> : <Table c={comp} rows={[]} own={own} />)}
        {tab === 'fixtures' && fixtures.length > 0 && <Calendar fixtures={fixtures} table={table} own={own} />}
        {tab === 'results' && <Results fixtures={fixtures} own={own} />}
        {comp && tab === 'history' && <History c={comp} />}
      </div>
      <BottomBar onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />
    </div>
  )
}
