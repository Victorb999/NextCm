import { useState } from 'react'
import type { ReactNode } from 'react'
import type { PersonProfile } from '../bindings/PersonProfile'
import type { StatLine } from '../bindings/StatLine'
import { cmDate } from '../format'
import { useNav } from '../nav'
import { Empty, Link, Panel, PurpleMenu } from './Screen'

// Aba Historial / Carreira do perfil, como no CM: o Vêr escolhe a Carreira (por época, com as
// estatísticas), as Lesões, as Suspensões ou o Currículo (títulos); por baixo, a época escolhida
// por tipo de competição, com o gráfico do rendimento.

type View = 'career' | 'injuries' | 'suspensions' | 'honours'

const VIEWS: { key: View; label: string }[] = [
  { key: 'career', label: 'Carreira do Jogador' },
  { key: 'injuries', label: 'Lesões' },
  { key: 'suspensions', label: 'Suspensões' },
  { key: 'honours', label: 'Currículo' },
]

const STAT_HEAD = ['Pres', 'Gol', 'Asts', 'HdJ', '%', 'Crt', 'Drb', 'Rem', 'Cla M']

/** As colunas de números de uma linha (o "-" quando não há dados). */
function statCells(s: StatLine): ReactNode[] {
  const dash = (v: number | null | undefined, f: (v: number) => string) => (v == null ? '-' : f(v))
  return [
    s.subs ? `${s.apps} (${s.subs})` : s.apps,
    s.goals,
    s.rating == null && s.apps ? '-' : s.assists,
    s.rating == null && s.apps ? '-' : s.mom,
    dash(s.passPct, (v) => `${v}%`),
    dash(s.tackles, (v) => v.toFixed(1)),
    dash(s.dribbles, (v) => v.toFixed(1)),
    dash(s.shotPct, (v) => `${v}%`),
    dash(s.rating, (v) => v.toFixed(2)),
  ]
}

const TH = 'bg-[#3d1a73] px-2 font-mono text-[11px] font-bold uppercase tracking-[0.1em] text-cm-yellow'

/** Tabela de cabeçalho roxo; `first` são as colunas de texto à esquerda. */
function Table({
  head,
  rows,
  first = 1,
  selected,
  onSelect,
  total,
}: {
  head: string[]
  rows: ReactNode[][]
  first?: number
  selected?: number
  onSelect?: (i: number) => void
  total?: ReactNode[]
}) {
  return (
    <table className="w-full border-collapse text-[13.5px]">
      <thead className="sticky top-0 z-10">
        <tr className="h-7">
          {head.map((h, i) => (
            <th key={i} className={`${TH} ${i < first ? 'text-left' : 'text-center'}`}>
              {h}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.map((r, i) => (
          <tr
            key={i}
            onClick={onSelect && (() => onSelect(i))}
            className={`h-[25px] ${onSelect ? 'cursor-pointer' : ''} ${i === selected ? '!bg-[#3a2a08]' : ''}`}
          >
            {r.map((c, j) => (
              <td key={j} className={`px-2 tabular-nums ${j < first ? 'text-left text-white' : 'text-center text-cm-orange'}`}>
                {c}
              </td>
            ))}
          </tr>
        ))}
        {total && (
          <tr className="h-[25px] !bg-[#1d4d3a]/50 font-bold">
            {total.map((c, j) => (
              <td key={j} className={`px-2 tabular-nums ${j < first ? 'text-left text-[#4edea3]' : 'text-center text-cm-yellow'}`}>
                {c}
              </td>
            ))}
          </tr>
        )}
      </tbody>
    </table>
  )
}

// Eixos do gráfico: o valor da época em 0–1 (o máximo é um jogador de topo nessa coluna)
const AXES: { label: string; value: (s: StatLine) => number }[] = [
  { label: 'Golos', value: (s) => (s.apps ? s.goals / s.apps / 0.8 : 0) },
  { label: 'Assist.', value: (s) => (s.apps ? s.assists / s.apps / 0.5 : 0) },
  { label: 'Passes', value: (s) => ((s.passPct ?? 40) - 40) / 55 },
  { label: 'Cortes', value: (s) => (s.tackles ?? 0) / 5 },
  { label: 'Dribles', value: (s) => (s.dribbles ?? 0) / 3 },
  { label: 'Remates', value: (s) => (s.shotPct ?? 0) / 80 },
  { label: 'Nota', value: (s) => ((s.rating ?? 6) - 6) / 2.2 },
]

/** O rendimento de uma época num gráfico de radar. */
function Radar({ s }: { s: StatLine }) {
  const size = 190
  const c = size / 2
  const r = 62
  const point = (i: number, v: number) => {
    const a = -Math.PI / 2 + (i * 2 * Math.PI) / AXES.length
    return [c + Math.cos(a) * r * v, c + Math.sin(a) * r * v]
  }
  const ring = (v: number) => AXES.map((_, i) => point(i, v).join(',')).join(' ')
  const shape = AXES.map((a, i) => point(i, Math.max(0.04, Math.min(1, a.value(s)))).join(',')).join(' ')
  return (
    <svg viewBox={`0 0 ${size} ${size}`} className="h-[132px] w-full">
      {[0.25, 0.5, 0.75, 1].map((v) => (
        <polygon key={v} points={ring(v)} fill="none" stroke="#2f6b52" strokeWidth="0.8" />
      ))}
      {AXES.map((_, i) => {
        const [x, y] = point(i, 1)
        return <line key={i} x1={c} y1={c} x2={x} y2={y} stroke="#2f6b52" strokeWidth="0.6" />
      })}
      <polygon points={shape} fill="#10b981" fillOpacity="0.35" stroke="#4edea3" strokeWidth="1.5" />
      {AXES.map((a, i) => {
        const [x, y] = point(i, 1.27)
        return (
          <text key={a.label} x={x} y={y} textAnchor="middle" dominantBaseline="middle" className="fill-[#c8c8d0] font-mono text-[9px]">
            {a.label}
          </text>
        )
      })}
    </svg>
  )
}

/** "3 semanas", "10 dias". */
function timeOut(days: number) {
  return days >= 14 && days % 7 < 4 ? `${Math.round(days / 7)} semanas` : `${days} dias`
}

/** Uma época por tipo de competição, com a linha dos jogos oficiais (também no Perfil Geral). */
export function SeasonTable({ byKind }: { byKind: [string, StatLine][] }) {
  return (
    <Table
      head={['Competição', ...STAT_HEAD]}
      rows={byKind.slice(0, -1).map(([k, s]) => [k, ...statCells(s)])}
      total={['Jogos Oficiais', ...statCells(byKind[byKind.length - 1][1])]}
    />
  )
}

export default function PlayerHistory({ p }: { p: PersonProfile }) {
  const nav = useNav()
  const [view, setView] = useState<View>('career')
  // A época escolhida para o quadro de baixo (por omissão a mais recente com dados do jogo)
  const played = p.seasons.filter((s) => s.byKind.length > 0)
  const [index, setIndex] = useState(0)
  const season = played[Math.min(index, played.length - 1)]

  const clubLink = (c: PersonProfile['club'], onLoan = false) => (
    <span key="club" className="text-cm-magenta">
      {c ? <Link onClick={() => nav.go({ kind: 'club', id: c.id })}>{c.shortName}</Link> : '-'}
      {onLoan && <span className="text-cm-yellow"> (emp.)</span>}
    </span>
  )

  // Fora de um jogo em curso, só a carreira do patch
  if (p.seasons.length === 0) {
    if (p.career.length === 0) return <Empty>Sem histórico registado.</Empty>
    return (
      <Panel title="Carreira do Jogador" className="mb-1">
        <div className="min-h-0 flex-1 overflow-auto">
          <Table
            head={['Época', 'Clube', 'Jogos', 'Golos']}
            first={2}
            rows={p.career.map((c) => [c.year ?? '-', clubLink(c.club, c.onLoan), c.apps, c.goals])}
          />
        </div>
      </Panel>
    )
  }

  const career = p.seasons.reduce(
    (t, s) => ({ apps: t.apps + s.stats.apps, goals: t.goals + s.stats.goals, assists: t.assists + s.stats.assists }),
    { apps: 0, goals: 0, assists: 0 },
  )
  const rated = p.seasons.filter((s) => s.stats.rating != null)
  const games = rated.reduce((t, s) => t + s.stats.apps, 0)
  const average = games ? rated.reduce((t, s) => t + (s.stats.rating ?? 0) * s.stats.apps, 0) / games : null
  const titles = p.honours.filter((h) => !h.text.startsWith('Finalista')).length

  let top: ReactNode
  if (view === 'career') {
    top = (
      <Table
        head={['Época', 'Clube', ...STAT_HEAD]}
        first={2}
        rows={p.seasons.map((s) => [s.label, clubLink(s.club, s.onLoan), ...statCells(s.stats)])}
        selected={season ? p.seasons.indexOf(season) : undefined}
        onSelect={(i) => {
          const k = played.indexOf(p.seasons[i])
          if (k >= 0) setIndex(k)
        }}
      />
    )
  } else if (view === 'injuries') {
    top = p.injuries.length ? (
      <Table
        head={['Data', 'Lesão', 'Tipo', 'Período Fora']}
        first={2}
        rows={p.injuries.map((l) => [cmDate(l.date), <span key="i" className="text-cm-yellow">{l.injury}</span>, l.kind, timeOut(l.days)])}
      />
    ) : (
      <Empty>Nenhuma lesão registada.</Empty>
    )
  } else if (view === 'suspensions') {
    top = p.suspensions.length ? (
      <Table
        head={['Data', 'Suspensão', 'Razão']}
        first={2}
        rows={p.suspensions.map((l) => [cmDate(l.date), <span key="t" className="text-cm-yellow">{l.text}</span>, l.reason])}
      />
    ) : (
      <Empty>Nenhuma suspensão registada.</Empty>
    )
  } else {
    top = p.honours.length ? (
      <Table
        head={['Data', 'Clube', 'Título']}
        first={3}
        rows={p.honours.map((h) => [cmDate(h.date), clubLink(h.club), <span key="t" className="text-cm-yellow">{h.text}</span>])}
      />
    ) : (
      <Empty>Ainda sem títulos.</Empty>
    )
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2.5 pb-1">
      <Panel
        title={VIEWS.find((v) => v.key === view)!.label}
        right={<PurpleMenu label="Vêr" align="right" options={VIEWS} value={view} onChange={setView} />}
        className="min-h-[140px] flex-1"
      >
        <div className="min-h-0 flex-1 overflow-auto">{top}</div>
      </Panel>
      {season && (
        <div className="flex shrink-0 gap-2.5">
          <Panel
            title={`${season.label} ${season.club?.shortName ?? ''}`}
            right={
              <span className="flex gap-1">
                <button className="cm-purple h-6 w-7" disabled={index >= played.length - 1} onClick={() => setIndex((i) => i + 1)}>
                  ◀
                </button>
                <button className="cm-purple h-6 w-7" disabled={index <= 0} onClick={() => setIndex((i) => Math.max(0, i - 1))}>
                  ▶
                </button>
              </span>
            }
            className="min-w-0 flex-1"
          >
            <SeasonTable byKind={season.byKind} />
          </Panel>
          <Panel title="Rendimento" className="w-[200px] shrink-0">
            <div className="flex flex-1 items-center justify-center">
              <Radar s={season.stats} />
            </div>
          </Panel>
        </div>
      )}
      <div className="flex shrink-0 flex-wrap items-center gap-x-5 rounded-md bg-[#1a1b21] px-3 py-1.5 font-mono text-[12px] uppercase tracking-wider text-[#86869a]">
        <span>
          Jogos na carreira: <b className="text-white">{career.apps}</b>
        </span>
        <span>
          Golos: <b className="text-cm-orange">{career.goals}</b>
        </span>
        <span>
          Assistências: <b className="text-[#4edea3]">{career.assists}</b>
        </span>
        <span>
          Títulos: <b className="text-cm-yellow">{titles}</b>
        </span>
        {average != null && (
          <span>
            Nota média: <b className="text-[#4edea3]">{average.toFixed(2)}</b>
          </span>
        )}
      </div>
    </div>
  )
}
