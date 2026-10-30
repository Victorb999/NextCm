import { Trophy } from 'lucide-react'
import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { api } from '../api'
import type { ClubRef } from '../bindings/ClubRef'
import type { CompetitionProfile } from '../bindings/CompetitionProfile'
import type { Fixture } from '../bindings/Fixture'
import type { StageView } from '../bindings/StageView'
import type { StandingRow } from '../bindings/StandingRow'
import type { TableZone } from '../bindings/TableZone'
import type { TieView } from '../bindings/TieView'
import { FixtureRows, RoundGames } from '../components/Fixtures'
import { BottomBar, Empty, ErrorBox, Link, Panel, ScreenHeader, Tabs } from '../components/Screen'
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

/** Cabeçalho de coluna roxo com letra amarela, como as tabelas do CM. */
const TH = 'bg-[#3d1a73] px-2 text-[11px] font-bold uppercase tracking-[0.1em] text-cm-yellow'

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
  if (c.history.length === 0) return <Empty>Sem historial registado.</Empty>
  const head = ['Ano', 'Campeão', 'Vice', '3º']
  return (
    <Panel title="Historial" className="flex-1">
      <div className="min-h-0 flex-1 overflow-auto">
        <table className="w-full border-collapse">
          <thead className="sticky top-0 z-10">
            <tr className="h-8">
              {head.map((h) => (
                <th key={h} className={`${TH} text-left`}>
                  {h}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {c.history.map((h, i) => (
              <tr key={i} className="h-[30px] text-[14.5px]">
                <td className="w-20 px-2 font-semibold tabular-nums text-cm-yellow">{h.year ?? '-'}</td>
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
    </Panel>
  )
}

const COLS = ['J', 'V', 'E', 'D', 'M', 'S', 'P']
// As cores das faixas da classificação (vagas, subida, descida), como as linhas do CM
const ZONE_COLOURS: Record<string, string> = {
  continental: '#1f9d3a',
  continental2: '#2f6fd6',
  continental3: '#16a3a3',
  promotion: '#1f9d3a',
  relegation: '#c02020',
}

/** A faixa do lugar `pos` (1 = primeiro) numa tabela de `n` clubes. */
function zoneAt(zones: TableZone[], pos: number, n: number) {
  return zones.find((z) => (z.fromBottom ? pos >= n - z.to + 1 && pos <= n - z.from + 1 : pos >= z.from && pos <= z.to))
}

const values = (r: StandingRow) => [r.played, r.won, r.drawn, r.lost, r.goalsFor, r.goalsAgainst, r.points]

/** A legenda das faixas da classificação. */
function ZoneLegend({ zones }: { zones: TableZone[] }) {
  return (
    <div className="flex flex-col gap-1.5 p-3 text-[13px] text-[#e3e1e9]">
      {zones.map((z) => (
        <span key={z.kind} className="flex items-center gap-2">
          <span className="inline-block h-3 w-1.5 rounded-sm" style={{ background: ZONE_COLOURS[z.kind] }} />
          {z.label}
        </span>
      ))}
    </div>
  )
}

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
    <Panel title={title} className="min-w-0 flex-1">
      <div className="min-h-0 flex-1 overflow-auto">
        <table className="w-full border-collapse">
          <thead className="sticky top-0 z-10">
            <tr className="h-8">
              <th className={`${TH} w-14 text-center`}>Pos</th>
              <th className={`${TH} text-left`}>Clube</th>
              {COLS.map((h) => (
                <th key={h} className={`${TH} w-12 text-center`}>
                  {h}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {shown.map((row, i) => {
              const zone = rows.length ? ZONE_COLOURS[zoneAt(c.zones, i + 1, shown.length)?.kind ?? ''] : undefined
              const mine = row.club.id === own
              return (
                <tr key={row.club.id} className={`h-[30px] text-[14.5px] ${mine ? 'bg-cm-pink/10' : ''}`}>
                  <td className="relative text-center font-semibold tabular-nums text-white">
                    {zone && <span className="absolute inset-y-1 left-0 w-1 rounded-r-sm" style={{ background: zone }} />}
                    {i + 1}º
                  </td>
                  <td className="px-2">
                    <Link
                      onClick={() => nav.go({ kind: 'club', id: row.club.id })}
                      className={`font-title text-[16px] ${mine ? 'font-bold text-cm-pink' : 'text-white'}`}
                    >
                      {row.club.shortName}
                    </Link>
                  </td>
                  {row.values.map((v, j) => (
                    <td key={COLS[j]} className={`text-center tabular-nums ${j === 6 ? 'font-bold text-cm-yellow' : 'text-[#e3e1e9]'}`}>
                      {v}
                    </td>
                  ))}
                </tr>
              )
            })}
          </tbody>
        </table>
      </div>
    </Panel>
  )
}

/** Um grupo em ponto pequeno, para caberem dois por linha. */
function GroupBox({ name, rows, own }: { name: string; rows: StandingRow[]; own?: number }) {
  const nav = useNav()
  return (
    <Panel title={name}>
      <table className="w-full border-collapse text-[14px]">
        <thead>
          <tr className="h-7">
            <th className={`${TH} text-left`}>Clube</th>
            {COLS.map((h) => (
              <th key={h} className={`${TH} w-8 text-center`}>
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.club.id} className={`h-[28px] ${r.club.id === own ? 'bg-cm-pink/10' : ''}`}>
              <td className="truncate px-2">
                <Link
                  onClick={() => nav.go({ kind: 'club', id: r.club.id })}
                  className={`font-title text-[15px] ${r.club.id === own ? 'font-bold text-cm-pink' : 'text-white'}`}
                >
                  {r.position}. {r.club.shortName}
                </Link>
              </td>
              {values(r).map((v, j) => (
                <td key={COLS[j]} className={`text-center tabular-nums ${j === 6 ? 'font-bold text-cm-yellow' : 'text-[#e3e1e9]'}`}>
                  {v}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </Panel>
  )
}

/** Uma eliminatória: os dois clubes, os resultados de cada mão, o total e os penáltis. */
function Tie({ tie, own }: { tie: TieView; own?: number }) {
  const nav = useNav()
  const name = (c: ClubRef, align: string) => (
    <Link
      onClick={() => nav.go({ kind: 'club', id: c.id })}
      className={`block truncate font-title text-[16px] ${align} ${tie.winner === c.id ? 'font-bold' : ''} ${
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
  const extra = [tie.aggregate ? `total ${tie.aggregate[0]}-${tie.aggregate[1]}` : '', tie.pens ? `${tie.pens[0]}-${tie.pens[1]} g.p.` : '']
    .filter(Boolean)
    .join(', ')
  return (
    <div className="flex h-[30px] items-center gap-2 px-3">
      <div className="min-w-0 flex-1">{name(tie.first, 'text-right')}</div>
      <span className="w-28 shrink-0 text-center font-bold tabular-nums text-cm-magenta">{legs.join(' · ')}</span>
      <div className="min-w-0 flex-1">{name(tie.second, 'text-left')}</div>
      <span className="w-36 shrink-0 text-right text-[13px] text-cm-orange">{extra}</span>
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

/** A jornada a mostrar primeiro: a próxima por jogar (ou a última). */
function startRound(rounds: [string, Fixture[]][]) {
  const next = rounds.findIndex(([, games]) => games.some((f) => f.homeGoals === null))
  return next < 0 ? rounds.length - 1 : next
}

/** ◀ jornada ▶, para o Calendário e o painel ao lado da classificação. */
function RoundPicker({ rounds, index, onChange }: { rounds: [string, Fixture[]][]; index: number; onChange: (i: number) => void }) {
  const [name, games] = rounds[index]
  return (
    <span className="flex items-center gap-1.5">
      <button className="cm-purple h-6 w-8 px-0" disabled={index <= 0} onClick={() => onChange(index - 1)} aria-label="Jornada anterior">
        ◀
      </button>
      <span className="min-w-[9rem] text-center font-bold text-cm-yellow">
        {name}
        {games[0] ? ` · ${matchDate(games[0].date).slice(4)}` : ''}
      </span>
      <button className="cm-purple h-6 w-8 px-0" disabled={index >= rounds.length - 1} onClick={() => onChange(index + 1)} aria-label="Jornada seguinte">
        ▶
      </button>
    </span>
  )
}

/** Painel "Jornada" ao lado da classificação: os jogos de uma jornada, com o resultado. */
function RoundPanel({ fixtures, own }: { fixtures: Fixture[]; own?: number }) {
  const nav = useNav()
  const rounds = byRound(fixtures)
  const [index, setIndex] = useState(() => startRound(rounds))
  if (rounds.length === 0) return null
  const i = Math.min(index, rounds.length - 1)
  const name = (c: ClubRef, align: string) => (
    <Link
      onClick={() => nav.go({ kind: 'club', id: c.id })}
      className={`block truncate ${align} ${c.id === own ? 'font-bold text-cm-yellow' : 'text-white'}`}
    >
      {c.shortName}
    </Link>
  )
  return (
    <Panel title="Jornada" right={<RoundPicker rounds={rounds} index={i} onChange={setIndex} />} className="min-h-0 flex-1">
      <div className="cm-zebra min-h-0 flex-1 overflow-auto">
        {rounds[i][1].map((f) => (
          <div key={f.id} className="flex h-[30px] items-center gap-2 px-3 text-[14px]">
            <div className="min-w-0 flex-1">{name(f.home, 'text-right')}</div>
            <span className="w-12 shrink-0 text-center font-bold tabular-nums text-cm-magenta">
              {f.homeGoals === null ? 'v' : `${f.homeGoals}:${f.awayGoals}`}
            </span>
            <div className="min-w-0 flex-1">{name(f.away, 'text-left')}</div>
          </div>
        ))}
      </div>
    </Panel>
  )
}

/** Classificação de uma competição viva, fase a fase (pontos corridos, grupos, eliminatórias). */
function Stages({ c, stages, fixtures, own }: { c: CompetitionProfile; stages: StageView[]; fixtures: Fixture[]; own?: number }) {
  const [index, setIndex] = useState(stages.length - 1)
  const stage = stages[Math.min(index, stages.length - 1)]
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2">
      {stages.length > 1 && (
        <div className="flex shrink-0 justify-center gap-1.5">
          {stages.map((st, i) => (
            <button key={st.name} className={`cm-purple h-7 w-44 ${i === index ? 'ring-1 ring-cm-yellow !text-cm-yellow' : ''}`} onClick={() => setIndex(i)}>
              {st.name}
            </button>
          ))}
        </div>
      )}
      {stage.kind === 'league' && (
        <div className="flex min-h-0 flex-1 gap-2">
          <Table c={c} rows={stage.tables[0]?.rows ?? []} own={own} title={stages.length > 1 ? stage.name : 'Classificação Geral'} />
          {(fixtures.length > 0 || c.zones.length > 0) && (
            <div className="flex min-h-0 w-[360px] shrink-0 flex-col gap-2">
              {fixtures.length > 0 && <RoundPanel fixtures={fixtures} own={own} />}
              {c.zones.length > 0 && (
                <Panel title="Zonas" className="shrink-0">
                  <ZoneLegend zones={c.zones} />
                </Panel>
              )}
            </div>
          )}
        </div>
      )}
      {stage.kind === 'groups' && (
        <div className="grid min-h-0 flex-1 grid-cols-2 content-start gap-2 overflow-auto">
          {stage.tables.map((t) => (
            <GroupBox key={t.name} name={t.name} rows={t.rows} own={own} />
          ))}
        </div>
      )}
      {stage.kind === 'knockout' && (
        <div className="flex min-h-0 flex-1 flex-col gap-2 overflow-auto">
          {[...stage.rounds].reverse().map((r) => (
            <Panel key={r.name} title={r.name} className="shrink-0">
              <div className="cm-zebra">
                {r.ties.map((t, i) => (
                  <Tie key={i} tie={t} own={own} />
                ))}
              </div>
            </Panel>
          ))}
        </div>
      )}
    </div>
  )
}

/** Calendário: uma jornada de cada vez, com ◀ ▶; abre na próxima por jogar. */
function Calendar({ fixtures, table, own }: { fixtures: Fixture[]; table: StandingRow[]; own?: number }) {
  const rounds = byRound(fixtures)
  const [index, setIndex] = useState(() => startRound(rounds))
  const i = Math.min(index, rounds.length - 1)
  return (
    <Panel title="Calendário" right={<RoundPicker rounds={rounds} index={i} onChange={setIndex} />} className="flex-1">
      <div className="min-h-0 flex-1 overflow-auto px-3 py-2">
        <RoundGames fixtures={rounds[i][1]} table={table} own={own} />
      </div>
    </Panel>
  )
}

/** Resultados: jornadas já disputadas, da mais recente para a mais antiga. */
function Results({ fixtures, own }: { fixtures: Fixture[]; own?: number }) {
  const rounds = byRound(fixtures.filter((f) => f.homeGoals !== null)).reverse()
  if (rounds.length === 0) return <Empty>Ainda não há resultados.</Empty>
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2 overflow-auto">
      {rounds.map(([name, games]) => (
        <Panel key={name} title={name} className="shrink-0">
          <FixtureRows fixtures={games} own={own} />
        </Panel>
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
      {/* O cabeçalho das outras telas; a taça no azul das competições do CM */}
      <ScreenHeader
        icon={
          <span className="flex size-11 items-center justify-center rounded bg-[#1e9cf0] text-white">
            <Trophy size={22} />
          </span>
        }
        eyebrow={
          comp && (
            <>
              <span className="rounded bg-[#0f2f4a] px-1.5 tracking-wider text-[#7cc8ff]">{where}</span>
              <span className="rounded bg-black/40 px-1.5 tracking-wider">Reputação {comp.reputation}</span>
              {comp.kind === 'national' && <span className="rounded bg-black/40 px-1.5 tracking-wider">Seleções</span>}
            </>
          )
        }
        title={comp?.name ?? ''}
      />
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
      <div className="mx-2 mt-2 flex min-h-0 flex-1 flex-col">
        <ErrorBox className="mb-2">{error}</ErrorBox>
        {comp &&
          tab === 'table' &&
          (stages.length > 0 ? <Stages c={comp} stages={stages} fixtures={fixtures} own={own} /> : <Table c={comp} rows={[]} own={own} />)}
        {tab === 'fixtures' && fixtures.length > 0 && <Calendar fixtures={fixtures} table={table} own={own} />}
        {tab === 'results' && <Results fixtures={fixtures} own={own} />}
        {comp && tab === 'history' && <History c={comp} />}
      </div>
      <BottomBar onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />
    </div>
  )
}
