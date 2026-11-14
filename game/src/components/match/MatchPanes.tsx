import { useEffect, useRef } from 'react'
import type { ReactNode } from 'react'
import type { ClubRef } from '../../bindings/ClubRef'
import type { LivePlayer } from '../../bindings/LivePlayer'
import type { LiveTeam } from '../../bindings/LiveTeam'
import type { LiveView } from '../../bindings/LiveView'
import { shortListName, xgText } from '../../format'
import { liveLineup } from '../../matchLineup'
import { conditionColour, MENTALITY, PASSING, TACKLING } from '../../tactics'
import PitchField from '../PitchField'
import MatchLineups from '../MatchLineups'
import { isGoalLine, pct, fullName, ratingColour } from './lines'
import type { Line } from './lines'
import { PossessionBar } from './Panorama'

export function Stats({ view }: { view: LiveView }) {
  const [h, a] = [view.home.stats, view.away.stats]
  const rows: [string, string | number, string | number, string?][] = [
    ['Remates à Baliza', h.shots, a.shots],
    ['No Alvo', h.onTarget, a.onTarget],
    ['Ao Lado', h.shots - h.onTarget, a.shots - a.onTarget],
    ['Golos Esperados', xgText(h.xg), xgText(a.xg)],
    ['Cantos', h.corners, a.corners],
    ['Livres', a.fouls, h.fouls],
    ['Lançamentos', h.throwIns, a.throwIns],
    ['Faltas', h.fouls, a.fouls],
    ['Foras de Jogo', h.offsides, a.offsides],
    ['Passes Acertados', pct(h.passesOk, h.passes), pct(a.passesOk, a.passes)],
    ['Cortes Ganhos', pct(h.tacklesWon, h.tackles), pct(a.tacklesWon, a.tackles)],
    ['Cabeceamentos Ganhos', pct(h.headersWon, h.headers), pct(a.headersWon, a.headers)],
    ['Cartões Amarelos', h.yellows, a.yellows, 'text-cm-yellow'],
    ['Cartões Vermelhos', h.reds, a.reds, 'text-cm-red'],
  ]
  return (
    <div className="px-2 py-1.5">
      {rows.map(([label, x, y, colour]) => (
        <div key={label} className="flex h-6 items-center gap-1">
          <span className="flex h-[25px] w-24 shrink-0 items-center justify-center bg-[#1a1a1a] font-title text-[18px] text-cm-magenta">
            {x}
          </span>
          <span className={`flex-1 text-center font-title text-[20px] ${colour ?? 'text-cm-text'}`}>{label}</span>
          <span className="flex h-[25px] w-24 shrink-0 items-center justify-center bg-[#1a1a1a] font-title text-[18px] text-cm-magenta">
            {y}
          </span>
        </div>
      ))}
    </div>
  )
}

export function Zones({ view }: { view: LiveView }) {
  const t = view.usefulTime
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 p-3">
      <div
        className="relative flex h-[60%] min-h-[200px] w-[78%] border-2 border-[#d8d8d8]/60"
        style={{
          background: 'repeating-linear-gradient(90deg, rgb(var(--c-pitch)) 0 44px, #126a1a 44px 88px)',
        }}
      >
        <div className="absolute left-0 top-1/4 h-1/2 w-[12%] border border-l-0 border-white/50" />
        <div className="absolute right-0 top-1/4 h-1/2 w-[12%] border border-r-0 border-white/50" />
        <div className="absolute left-1/2 top-1/2 h-20 w-20 -translate-x-1/2 -translate-y-1/2 rounded-full border border-white/50" />
        <div className="absolute left-1/2 top-0 h-full border-l border-white/40" />
        <div className="absolute left-1/2 top-2 -translate-x-1/2 bg-black/60 px-12 text-white">Global</div>
        <div className="absolute right-3 top-2 text-[13px] text-cm-yellow">
          Tempo útil : {Math.floor(t / 60)}' {t % 60}
        </div>
        {[0, 1, 2].map((i) => (
          <div
            key={i}
            className={`relative flex flex-1 flex-col items-center justify-center ${i < 2 ? 'border-r border-dashed border-white/80' : ''}`}
          >
            <span className="font-title text-[26px] font-bold text-cm-yellow">{view.zones[i]}%</span>
            <div className="absolute bottom-2 left-2 right-2 flex h-4 border-2 border-[#3a1470] bg-white">
              <div
                style={{
                  width: `${view.zonePossession[i]}%`,
                  background: view.home.club.bg ?? '#2020c0',
                }}
              />
            </div>
          </div>
        ))}
      </div>
      <div className="w-[78%]">
        <PossessionBar view={view} home={view.home.stats.possession} label="Posse de bola" />
      </div>
    </div>
  )
}

export function Report({ lines }: { lines: Line[] }) {
  const box = useRef<HTMLDivElement>(null)
  // A frase mais recente fica à vista (só o painel rola)
  useEffect(() => {
    const panel = box.current?.parentElement
    if (panel) panel.scrollTop = panel.scrollHeight
  }, [lines.length])
  return (
    <div ref={box} className="px-2 py-1">
      {lines.map((l, i) => (
        <div key={i} className="flex h-[19px] items-center border-b border-white/5 text-[13px]">
          <span className="w-24 shrink-0 pl-6 font-bold text-cm-yellow">{lines[i - 1]?.minute !== l.minute ? `${l.minute} min` : ''}</span>
          <span
            className={`truncate ${l.kind === 'goal' ? 'font-bold text-cm-yellow' : l.kind === 'red' ? 'text-cm-red' : 'text-cm-text'}`}
          >
            {l.text}
          </span>
        </div>
      ))}
    </div>
  )
}

/** O campo dos Dados da equipa, deitado (o ataque para a direita): quem está em campo no seu
 * lugar, com o número e a nota na cor. */
function TeamPitch({ team }: { team: LiveTeam }) {
  const on = team.players.filter((p) => p.x !== null && p.y !== null && !p.red)
  return (
    <div className="flex min-w-0 flex-[3] flex-col">
      <div className="h-[18px] bg-cm-head px-2 text-[12px] font-bold uppercase leading-[18px] tracking-wide text-white">
        Em Campo · {team.formation}
      </div>
      <PitchField landscape className="mt-1 w-full">
        {on.map((p) => (
          <div
            key={p.id}
            className="absolute flex -translate-x-1/2 -translate-y-1/2 flex-col items-center"
            // De pé o ataque é para cima (y pequeno); deitado, para a direita
            style={{ left: `${100 - (p.y ?? 0)}%`, top: `${p.x}%` }}
            title={`${fullName(p)} · ${p.role ?? ''}`}
          >
            <span
              className="flex size-7 items-center justify-center rounded-md text-[13px] font-bold text-[#141414] shadow-[0_2px_6px_rgba(0,0,0,0.5)]"
              style={{ background: ratingColour(p.rating) }}
            >
              {p.squadNumber ?? ''}
            </span>
            <span className="mt-0.5 whitespace-nowrap rounded bg-black/75 px-1 text-[11px] font-bold leading-[15px] text-white">
              {shortListName(p.listName)}
              {p.rating !== null && <span style={{ color: ratingColour(p.rating) }}> {p.rating}</span>}
              {p.goals > 0 && <span className="text-cm-yellow"> {'●'.repeat(p.goals)}</span>}
              {p.yellow && <span className="text-cm-yellow"> ▮</span>}
            </span>
          </div>
        ))}
      </PitchField>
    </div>
  )
}

/** As notas de quem jogou em barras, da melhor para a pior (com os golos e os cartões). */
function RatingBars({ players }: { players: LivePlayer[] }) {
  const rated = players.filter((p) => p.rating !== null).sort((a, b) => (b.rating ?? 0) - (a.rating ?? 0))
  return (
    <div className="flex min-w-[220px] flex-[2] flex-col">
      <div className="h-[18px] bg-cm-head px-2 text-[12px] font-bold uppercase leading-[18px] tracking-wide text-white">Notas</div>
      <div className="mt-1 flex flex-col gap-[3px]">
        {rated.map((p) => (
          <div key={p.id} className="flex h-[19px] items-center gap-2 text-[13px]">
            <span className="w-32 shrink-0 truncate text-white">{shortListName(p.listName)}</span>
            <div className="relative h-[13px] min-w-0 flex-1 rounded-sm bg-black/40">
              <div
                className="h-full rounded-sm"
                style={{ width: `${((p.rating ?? 0) / 10) * 100}%`, background: ratingColour(p.rating) }}
              />
            </div>
            <span className="w-5 shrink-0 text-right font-bold" style={{ color: ratingColour(p.rating) }}>
              {p.rating}
            </span>
            <span className="w-10 shrink-0 truncate text-cm-yellow">
              {p.goals > 0 ? '●'.repeat(p.goals) : ''}
              {p.red ? <span className="text-cm-red"> ▮</span> : p.yellow ? ' ▮' : ''}
            </span>
          </div>
        ))}
      </div>
    </div>
  )
}

/** Uma caixa dos Dados da equipa: o título e as linhas "rótulo ... valor". */
function DataBox({ title, rows }: { title: string; rows: [string, ReactNode][] }) {
  return (
    <div className="min-w-0 flex-1 rounded bg-black/25">
      <div className="h-[20px] bg-cm-head px-2 text-[12px] font-bold uppercase leading-[20px] tracking-wide text-white">{title}</div>
      <div className="cm-zebra">
        {rows.map(([label, value]) => (
          <div key={label} className="flex h-[19px] items-center gap-2 px-2 text-[13px]">
            <span className="truncate text-[#c9c9d6]">{label}</span>
            <span className="ml-auto shrink-0 truncate text-cm-yellow">{value}</span>
          </div>
        ))}
      </div>
    </div>
  )
}

/** "Dados <clube>": os números de cada jogador, os totais, os da equipa, os destaques e a táctica. */
export function TeamData({ team }: { team: LiveTeam }) {
  const cols = ['Pas', 'Cmp', 'Crt', 'Gnh', 'Cab', 'Gnh', 'Dri', 'Ast', 'Rem', 'Bal', 'Con', 'Nt', 'G']
  const played = [...team.players, ...team.out]
  const sum = (f: (p: LivePlayer) => number) => played.reduce((n, p) => n + f(p), 0)
  // O melhor de cada coisa entre quem jogou ("-" se ninguém tem nada)
  const best = (f: (p: LivePlayer) => number, show: (p: LivePlayer) => string = (p) => String(f(p))) => {
    const top = played.reduce<LivePlayer | null>((b, p) => (f(p) > 0 && (!b || f(p) > f(b)) ? p : b), null)
    return top ? `${shortListName(top.listName)} (${show(top)})` : '-'
  }
  const s = team.stats
  const onPitch = team.players.filter((p) => !p.red)
  const condition = onPitch.length ? Math.round(onPitch.reduce((n, p) => n + p.condition, 0) / onPitch.length) : 0
  const scorers = played.filter((p) => p.goals > 0)
  const i = team.instructions
  const yes = (v: boolean) => (v ? 'Sim' : 'Não')
  const row = (p: LivePlayer, played: boolean) => {
    const s = p.stats
    const vals = played
      ? [s.passes, s.passesOk, s.tackles, s.tacklesWon, s.headers, s.headersWon, s.dribbles, p.assists, s.shots, s.onTarget]
      : []
    return (
      <div key={p.id} className="flex h-[19px] items-center gap-1 text-[13px]">
        <span
          className={`h-full w-7 shrink-0 text-center leading-[19px] ${played ? 'bg-[#262626] text-white' : 'bg-[#161660] text-cm-lilac'}`}
        >
          {p.squadNumber ?? ''}
        </span>
        <span className="flex w-3 shrink-0 justify-center">
          {p.red ? <span className="h-3 w-2 bg-cm-red" /> : p.yellow ? <span className="h-3 w-2 bg-cm-yellow" /> : null}
        </span>
        <span
          className={`w-48 shrink-0 truncate ${!played ? 'text-cm-purplelight' : p.offMinute !== null && !p.red ? 'text-cm-lilac' : 'text-white'}`}
        >
          {fullName(p)}
          {p.offMinute !== null && !p.red ? <span className="text-cm-red"> ↓{p.offMinute}'</span> : null}
          {p.onMinute !== null ? <span className="text-cm-green"> ↑{p.onMinute}'</span> : null}
        </span>
        {vals.map((v, i) => (
          <span key={i} className={`w-9 shrink-0 text-center ${i < 2 ? 'text-cm-orange' : 'text-cm-yellow'}`}>
            {v}
          </span>
        ))}
        {played && (
          <>
            <span className="w-9 shrink-0 text-center text-[#40a0ff]">{p.condition}</span>
            <span className="w-9 shrink-0 text-center text-cm-magenta">{p.rating ?? ''}</span>
            <span className="w-9 shrink-0 text-center text-white">{p.goals || ''}</span>
          </>
        )}
      </div>
    )
  }
  return (
    <div className="px-3 py-2">
      <div className="flex gap-3">
        <div className="shrink-0">
          <div className="mb-1 flex h-[18px] items-center gap-1 text-[12px]">
            <span className="w-7 shrink-0" />
            <span className="w-3 shrink-0" />
            <span className="w-48 shrink-0" />
            {cols.map((c, i) => (
              <span key={i} className="w-9 shrink-0 bg-cm-head text-center font-bold text-white">
                {c}
              </span>
            ))}
          </div>
          <div className="cm-zebra">
            {played.map((p) => row(p, true))}
            {team.bench.map((p) => row(p, false))}
          </div>
          {/* Os totais de quem jogou, por baixo das colunas */}
          <div className="flex h-[20px] items-center gap-1 border-t border-white/20 text-[13px] font-bold">
            <span className="w-7 shrink-0" />
            <span className="w-3 shrink-0" />
            <span className="w-48 shrink-0 text-white">Totais</span>
            {[
              s.passes,
              s.passesOk,
              s.tackles,
              s.tacklesWon,
              s.headers,
              s.headersWon,
              sum((p) => p.stats.dribbles),
              sum((p) => p.assists),
              s.shots,
              s.onTarget,
            ].map((v, k) => (
              <span key={k} className={`w-9 shrink-0 text-center ${k < 2 ? 'text-cm-orange' : 'text-cm-yellow'}`}>
                {v}
              </span>
            ))}
            <span className="w-9 shrink-0 text-center text-[#40a0ff]">{condition || ''}</span>
            <span className="w-9 shrink-0" />
            <span className="w-9 shrink-0 text-center text-white">{sum((p) => p.goals) || ''}</span>
          </div>
        </div>
        {/* Ao lado da tabela: o campo com quem está em campo e as notas */}
        <TeamPitch team={team} />
        <RatingBars players={played} />
      </div>
      <div className="mt-2 flex gap-2">
        <DataBox
          title="Equipa"
          rows={[
            ['Posse de Bola', `${s.possession}%`],
            ['Remates (à baliza)', `${s.shots} (${s.onTarget})`],
            ['Golos Esperados', xgText(s.xg)],
            ['Passes Acertados', pct(s.passesOk, s.passes)],
            ['Cortes Ganhos', pct(s.tacklesWon, s.tackles)],
            ['Cabeceamentos Ganhos', pct(s.headersWon, s.headers)],
            ['Dribles', sum((p) => p.stats.dribbles)],
            ['Cantos', s.corners],
            ['Faltas', s.fouls],
            ['Foras de Jogo', s.offsides],
            ['Cartões', `${s.yellows} ${s.yellows === 1 ? 'amarelo' : 'amarelos'} · ${s.reds} ${s.reds === 1 ? 'vermelho' : 'vermelhos'}`],
          ]}
        />
        <DataBox
          title="Destaques"
          rows={[
            ['Melhor Nota', best((p) => p.rating ?? 0)],
            [
              'Golos',
              scorers.length ? scorers.map((p) => `${shortListName(p.listName)}${p.goals > 1 ? ` ${p.goals}` : ''}`).join(', ') : '-',
            ],
            ['Assistências', best((p) => p.assists)],
            ['Mais Remates', best((p) => p.stats.shots)],
            [
              'Mais Golos Esperados',
              best(
                (p) => p.stats.xg,
                (p) => xgText(p.stats.xg),
              ),
            ],
            ['Mais Passes', best((p) => p.stats.passes)],
            [
              'Melhor Passe',
              best(
                (p) => (p.stats.passes >= 10 ? p.stats.passesOk / p.stats.passes : 0),
                (p) => pct(p.stats.passesOk, p.stats.passes),
              ),
            ],
            ['Mais Cortes Ganhos', best((p) => p.stats.tacklesWon)],
            ['Mais Dribles', best((p) => p.stats.dribbles)],
            ['Jogo Aéreo', best((p) => p.stats.headersWon)],
            [
              'Menos Condição',
              onPitch.length
                ? best(
                    (p) => 100 - p.condition,
                    (p) => `${p.condition}%`,
                  )
                : '-',
            ],
          ]}
        />
        <DataBox
          title="Táctica"
          rows={[
            ['Formação', team.formation],
            ['Mentalidade', MENTALITY.find(([k]) => k === i.mentality)?.[1] ?? '-'],
            ['Passe', PASSING.find(([k]) => k === i.passing)?.[1] ?? '-'],
            ['Desarme', TACKLING.find(([k]) => k === i.tackling)?.[1] ?? '-'],
            ['Pressão', yes(i.pressing)],
            ['Defesa em Linha', yes(i.offsideTrap)],
            ['Contra-Ataque', yes(i.counterAttack)],
            ['Defesa Intensa', yes(i.tightMarking)],
            ['Substituições', `${team.subsLeft} por fazer`],
            ['Condição Média', condition ? `${condition}%` : '-'],
          ]}
        />
      </div>
    </div>
  )
}

/** "Classificações Jogadores": as duas listas como no CM (quem saiu fica no lugar, quem entrou em baixo). */
export function Ratings({ view, motm }: { view: LiveView; motm: number | null }) {
  return (
    <MatchLineups
      home={{ club: view.home.club, entries: liveLineup(view.home) }}
      away={{ club: view.away.club, entries: liveLineup(view.away) }}
      motm={motm}
    />
  )
}

export function Results({ view, own }: { view: LiveView; own: number }) {
  const name = (c: ClubRef) => (
    <span className={`flex-1 truncate font-title text-[20px] ${c.id === own ? 'text-cm-yellow' : 'text-white'}`}>{c.shortName}</span>
  )
  if (view.others.length === 0) return <div className="p-4 text-[#aab3c4]">Não há outros jogos hoje.</div>
  return (
    <div className="px-6 pt-3 cm-zebra">
      <div className="mb-2 text-center text-[13px] text-cm-pink">
        {view.fixture.competition} · {view.minute}'
      </div>
      {view.others.map((o) => (
        <div key={o.fixture.id} className="flex h-[30px] items-center gap-3">
          {name(o.fixture.home)}
          <span className="w-16 shrink-0 text-center font-title text-[20px] font-bold text-cm-magenta">
            {o.homeGoals}:{o.awayGoals}
          </span>
          {name(o.fixture.away)}
        </div>
      ))}
    </div>
  )
}

export function Table({ view, own }: { view: LiveView; own: number }) {
  const playing = [view.home.club.id, view.away.club.id]
  const heads = ['J', 'V', 'E', 'D', 'M', 'S', 'P']
  return (
    <div className="px-3 py-2 text-[13px]">
      <div className="flex h-5 items-center bg-cm-head font-bold text-cm-yellow">
        <span className="w-10 text-center" />
        <span className="flex-1">Clube</span>
        {heads.map((h) => (
          <span key={h} className="w-10 text-center">
            {h}
          </span>
        ))}
      </div>
      {view.table.map((r) => (
        <div
          key={r.club.id}
          className={`flex h-[19px] items-center border-b border-cm-line ${playing.includes(r.club.id) ? 'bg-white/10' : ''}`}
        >
          <span className="w-10 text-center text-white">{r.position}º</span>
          <span className={`flex-1 truncate ${r.club.id === own ? 'text-cm-pink' : 'text-white'}`}>{r.club.shortName}</span>
          {[r.played, r.won, r.drawn, r.lost, r.goalsFor, r.goalsAgainst, r.points].map((v, i) => (
            <span key={i} className={`w-10 text-center ${i === 6 ? 'font-bold text-cm-yellow' : 'text-cm-text'}`}>
              {v}
            </span>
          ))}
        </div>
      ))}
    </div>
  )
}

/** "<clube> · Destaques": os três com melhor nota em campo, com a condição. */
export function Highlights({ team }: { team: LiveTeam }) {
  const top = [...team.players].sort((a, b) => (b.rating ?? 0) - (a.rating ?? 0)).slice(0, 3)
  return (
    <div className="min-w-0 flex-1 rounded bg-cm-surface px-2.5 py-2">
      <div className="mb-1 flex items-center justify-between text-[12px] font-bold font-mono uppercase tracking-wider">
        <span className="truncate text-cm-orange">{team.club.shortName} · Destaques</span>
        <span className="shrink-0 text-cm-dim">Con / Nota</span>
      </div>
      {top.map((p) => (
        <div key={p.id} className="flex h-6 items-center gap-2 text-[13px]">
          <span className="min-w-0 flex-1 truncate text-white">
            {shortListName(p.listName)} <span className="text-cm-dim">({p.role ?? p.position})</span>
          </span>
          <span className={`shrink-0 font-bold ${conditionColour(p.condition)}`}>{p.condition}%</span>
          <span className="w-6 shrink-0 rounded bg-[#34343a] text-center font-bold text-white">{p.rating ?? '-'}</span>
        </div>
      ))}
    </div>
  )
}

/** Os últimos lances que contam (golos, defesas, cartões, substituições), o mais recente em cima. */
export function History({ said }: { said: Line[] }) {
  const kinds = ['save', 'miss', 'yellow', 'red', 'injury', 'sub']
  const recent = said
    .filter((l) => isGoalLine(l) || (l.last && kinds.includes(l.kind)))
    .slice(-3)
    .reverse()
  return (
    <div className="min-w-0 flex-[1.3] rounded bg-cm-surface px-2.5 py-2">
      <div className="mb-1 text-[12px] font-bold font-mono uppercase tracking-wider text-cm-orange">Histórico Recente</div>
      {recent.length === 0 && <div className="text-[13px] text-cm-dim">Ainda sem lances.</div>}
      {recent.map((l, i) => (
        <div key={i} className="flex h-6 items-center gap-2 text-[13px]">
          <span className="w-8 shrink-0 font-bold text-cm-orange">{l.minute}'</span>
          <span className={`truncate ${isGoalLine(l) ? 'font-bold text-cm-yellow' : l.kind === 'red' ? 'text-cm-red' : 'text-cm-text'}`}>
            {l.text}
          </span>
        </div>
      ))}
    </div>
  )
}
