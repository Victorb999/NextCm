import { useState } from 'react'
import { api } from '../../api'
import type { LiveView } from '../../bindings/LiveView'
import type { MatchSheet } from '../../bindings/MatchSheet'
import { longDate, pensText, xgText } from '../../format'
import { liveLineup, sheetLineup } from '../../matchLineup'
import { useNav } from '../../nav'
import { useLoad } from '../../useLoad'
import MatchLineups from '../MatchLineups'
import { Empty, Link, Tabs } from '../Screen'
import { Report, Stats, TeamData, Zones } from './MatchPanes'
import { ShootoutList } from './ShootoutList'
import type { Line } from './lines'

// Uma partida jogada como o CM a mostra depois (ao clicar num resultado, no Último Jogo do
// clube): o placar e, em cima, Visão Geral / Estatísticas / Zonas de Ação / Relatório; em baixo,
// Dados de cada clube e as Classificações Jogadores. Os jogos de antes do save 74 só têm as
// notas; os das reservas (motor rápido) não têm estatísticas, zonas nem relatório.

type Pane = 'overview' | 'stats' | 'zones' | 'report' | 'homeData' | 'ratings' | 'awayData'

/** O placar por tempos, como no CM: "INT 0-0", e num mata-mata decidido depois dos 90'
 * "INT 0-0 FJ 1-1 Prorr 1-1 Pên 2-4" (FJ = fim do jogo). */
function periods(v: LiveView) {
  const at = (minute: number) => {
    const goals = (side: number) => v.events.filter((e) => e.kind === 'goal' && e.side === side && e.minute <= minute).length
    return `${goals(0)}-${goals(1)}`
  }
  let out = `INT ${at(45)}`
  if (v.extraTime || v.homePens !== null) out += ` FJ ${at(90)}`
  if (v.extraTime) out += ` Prorr ${at(120)}`
  if (v.homePens !== null && v.awayPens !== null) out += ` Pên ${v.homePens}-${v.awayPens}`
  return out
}

/** Os marcadores de um lado: com o minuto quando a partida está guardada, senão só o nome. */
function scorers(sheet: MatchSheet, side: 0 | 1): [string, string][] {
  const v = sheet.report
  if (v) {
    const team = side === 0 ? v.home : v.away
    const all = [...team.players, ...team.out, ...team.bench]
    return v.events
      .filter((e) => e.kind === 'goal' && e.side === side)
      .map((e) => {
        const p = all.find((p) => p.id === e.playerId)
        const name = p ? p.listName.split(', ').reverse().join(' ') : e.text
        return [name, `${e.minute}`]
      })
  }
  return (side === 0 ? sheet.home : sheet.away)
    .filter((l) => l.goals > 0)
    .map((l) => [l.name.split(', ').reverse().join(' '), l.goals > 1 ? `${l.goals} gols` : ''])
}

function Overview({ sheet }: { sheet: MatchSheet }) {
  const f = sheet.fixture
  const v = sheet.report
  const nav = useNav()
  const place = v ? [v.venue, v.city].filter(Boolean).join(', ') : ''
  return (
    <div className="flex h-full flex-col">
      {place && <div className="pt-3 text-center font-title text-[22px] font-bold text-cm-yellow">{place}</div>}
      <div className="mx-3 mt-2 flex h-7 items-center justify-between rounded bg-black/30 px-3 text-[14px] text-cm-pink">
        <span>{longDate(f.date)}</span>
        <span>
          <Link onClick={() => nav.go({ kind: 'competition', id: f.competitionId })}>{f.competition}</Link>
          {v && ` · ${periods(v)}`}
        </span>
      </div>
      <div className="mx-3 mt-3 flex gap-6">
        {([0, 1] as const).map((side) => (
          <div key={side} className="min-w-0 flex-1">
            {scorers(sheet, side).map(([name, minute], i) => (
              <div key={i} className="flex h-8 items-center gap-3 font-title text-[22px]">
                <span className="min-w-0 flex-1 truncate text-white">{name}</span>
                <span className="w-16 shrink-0 text-right font-bold text-cm-yellow">{minute}</span>
              </div>
            ))}
          </div>
        ))}
      </div>
      {v ? (
        <div className="mt-3">
          <ShootoutList view={v} kicks={v.events.filter((e) => e.kind === 'pen' || e.kind === 'penmiss')} />
        </div>
      ) : (
        pensText(f) && <div className="mt-2 text-center text-[14px] text-cm-yellow">{pensText(f)}</div>
      )}
      {v && (v.referee || v.attendance > 0 || v.weather) && (
        <div className="mx-3 mb-3 mt-auto flex justify-between text-[14px]">
          <span className="text-cm-yellow">{v.referee && `Árbitro: ${v.referee}`}</span>
          <span className="text-cm-pink">{v.attendance > 0 && `Público: ${v.attendance.toLocaleString('pt-BR')}`}</span>
          <span className="text-cm-yellow">{v.weather && `Tempo: ${v.weather}`}</span>
        </div>
      )}
      {!v && (
        <div className="mx-3 mb-3 mt-auto text-[13px] text-cm-dim">
          Partida de uma versão antiga, que não guardava os jogos: só tem as notas.
        </div>
      )}
    </div>
  )
}

/** O Relatório: os lances que contam (gols, cartões, substituições, lesões), pelo minuto. */
const reportLines = (v: LiveView): Line[] =>
  v.events
    .filter((e) => !['start', 'half', 'end'].includes(e.kind))
    .map((e) => ({ minute: e.minute, side: e.side, text: e.text, kind: e.kind, playerId: e.playerId, last: true }))

export function MatchHistoryBody({ sheet }: { sheet: MatchSheet }) {
  const [pane, setPane] = useState<Pane>('overview')
  const f = sheet.fixture
  const v = sheet.report
  // Os jogos das reservas não têm estatísticas (motor rápido)
  const detailed = !!v && v.usefulTime > 0
  const motm = [...sheet.home, ...sheet.away].find((l) => l.mom)?.id ?? null
  // Os gols esperados: os do time na partida guardada; senão a soma dos de quem jogou
  const xg: [number, number] =
    v && detailed
      ? [v.home.stats.xg, v.away.stats.xg]
      : [sheet.home.reduce((n, l) => n + l.xg, 0), sheet.away.reduce((n, l) => n + l.xg, 0)]
  const band = (club: MatchSheet['fixture']['home'], goals: number | null, right: boolean) => (
    <div
      className={`flex h-14 flex-1 items-center gap-3 rounded-md px-4 ${right ? 'flex-row-reverse' : ''}`}
      style={{ color: club.fg ?? '#fff', background: club.bg ?? '#333' }}
    >
      <span className="min-w-0 flex-1 truncate font-title text-[26px] font-bold" style={{ textAlign: right ? 'right' : 'left' }}>
        {club.shortName}
      </span>
      <span className="flex h-10 w-12 items-center justify-center rounded bg-black/80 font-title text-[26px] font-bold text-white">
        {goals ?? ''}
      </span>
    </div>
  )
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2">
      <div className="flex shrink-0 gap-2">
        {band(f.home, f.homeGoals, false)}
        {band(f.away, f.awayGoals, true)}
      </div>
      {xg[0] + xg[1] > 0 && (
        <div className="flex shrink-0 items-center justify-center gap-3 text-[14px]">
          <span className="w-14 rounded bg-black/60 text-center font-bold text-cm-magenta">{xgText(xg[0])}</span>
          <span className="text-cm-text">Gols Esperados</span>
          <span className="w-14 rounded bg-black/60 text-center font-bold text-cm-magenta">{xgText(xg[1])}</span>
        </div>
      )}
      <Tabs
        tabs={[
          { key: 'overview', label: 'Visão Geral' },
          { key: 'stats', label: 'Estatísticas', disabled: !detailed },
          { key: 'zones', label: 'Zonas de Ação', disabled: !detailed },
          { key: 'report', label: 'Relatório', disabled: !v },
        ]}
        active={pane}
        onChange={(k) => setPane(k as Pane)}
      />
      <div className="min-h-0 flex-1 overflow-auto rounded-md bg-cm-surface">
        {pane === 'overview' && <Overview sheet={sheet} />}
        {pane === 'stats' && v && <Stats view={v} />}
        {pane === 'zones' && v && <Zones view={v} />}
        {pane === 'report' && v && <Report lines={reportLines(v)} />}
        {pane === 'homeData' && v && <TeamData team={v.home} />}
        {pane === 'awayData' && v && <TeamData team={v.away} />}
        {pane === 'ratings' && (
          <div className="p-1">
            <MatchLineups
              home={{ club: f.home, entries: v ? liveLineup(v.home) : sheetLineup(sheet.home) }}
              away={{ club: f.away, entries: v ? liveLineup(v.away) : sheetLineup(sheet.away) }}
              motm={motm}
            />
          </div>
        )}
      </div>
      <Tabs
        tabs={[
          { key: 'homeData', label: `Dados ${f.home.shortName}`, disabled: !v },
          { key: 'ratings', label: 'Classificações Jogadores' },
          { key: 'awayData', label: `Dados ${f.away.shortName}`, disabled: !v },
        ]}
        active={pane}
        onChange={(k) => setPane(k as Pane)}
      />
    </div>
  )
}

/** A partida jogada `id`, carregada (a ficha e a aba Último Jogo do clube). */
export default function MatchHistory({ id }: { id: number }) {
  const [sheet, setSheet] = useState<MatchSheet | null>(null)
  const [error, setError] = useState<string | null>(null)
  useLoad(() => api.matchSheet(id), [id], setSheet, setError)
  if (error) return <Empty>{error}</Empty>
  if (!sheet || sheet.fixture.id !== id) return null
  return <MatchHistoryBody key={id} sheet={sheet} />
}
