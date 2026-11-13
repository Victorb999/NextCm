import type { ClubRef } from '../../bindings/ClubRef'
import type { LiveView } from '../../bindings/LiveView'
import { initials, longDate } from '../../format'
import { surname } from '../../tactics'
import { clubStyle, clubBand, isGoalLine, lineLabel, manOfTheMatch } from './lines'
import type { Line } from './lines'

/** Barra de posse no caixilho roxo, com as cores dos clubes e os nomes por cima. */
export function PossessionBar({ view, home, label }: { view: LiveView; home: number; label: string }) {
  const seg = (c: ClubRef, pct: number, align: string) => (
    <div className={`flex items-center px-2 text-[11px] font-bold ${align}`} style={{ ...clubStyle(c), width: `${pct}%` }}>
      {pct > 8 ? `${pct}%` : ''}
    </div>
  )
  return (
    <div className="flex flex-col gap-1.5 rounded border border-cm-purple/60 bg-cm-purple/25 px-3 py-2">
      <div className="flex items-center justify-between text-[12px] font-bold font-mono uppercase tracking-wider">
        <span className="text-cm-lilac">{label}</span>
        <span className="normal-case tracking-normal text-cm-text">
          {view.home.club.shortName} {home}% <span className="text-cm-dim">vs</span> {view.away.club.shortName} {100 - home}%
        </span>
      </div>
      <div className="flex h-4 overflow-hidden rounded bg-black">
        {seg(view.home.club, home, 'justify-end')}
        <div className="w-1 shrink-0 bg-cm-orange" />
        {seg(view.away.club, 100 - home, 'justify-start')}
      </div>
    </div>
  )
}

/** A frase do relato em destaque: cartão branco com a cor de quem a faz; o golo nas cores do clube. */
function LineCard({ view, line }: { view: LiveView; line: Line }) {
  const team = line.side === 0 ? view.home : line.side === 1 ? view.away : null
  const goal = isGoalLine(line) && team
  const stripe = team?.club.bg ?? `linear-gradient(90deg, ${view.home.club.bg ?? '#888'}, #888, ${view.away.club.bg ?? '#888'})`
  return (
    <div
      className={`relative w-full max-w-[760px] overflow-hidden rounded-lg px-5 pb-3.5 pt-4
        shadow-[0_12px_32px_rgba(0,0,0,0.8)] ${goal ? 'cm-goal-in' : 'cm-line-in bg-white text-[#141414]'}`}
      style={team ? clubBand(team.club) : { background: stripe }}
    >
      {!goal && <div className="absolute inset-x-0 top-0 h-1" style={{ background: stripe }} />}
      <div className="flex items-center gap-4">
        {team && (
          <span
            className="flex size-10 shrink-0 items-center justify-center rounded-full border-2 border-black/20 font-title text-[14px] font-bold"
            style={clubStyle(team.club)}
          >
            {initials(team.club.shortName)}
          </span>
        )}
        <div className="min-w-0 flex-1">
          <div className="text-[11px] font-bold font-mono uppercase tracking-widest opacity-60">
            {lineLabel(line)} · {line.minute}'
          </div>
          <div className={`font-title text-[24px] leading-tight ${goal ? 'font-bold' : ''}`}>{line.text}</div>
        </div>
      </div>
    </div>
  )
}

/** As linhas do campo, apagadas, por trás do Panorama. */
function PitchLines() {
  return (
    <div className="pointer-events-none absolute inset-x-6 bottom-28 top-10 border border-white/[0.07]">
      <div className="absolute inset-y-0 left-1/2 border-l border-white/[0.07]" />
      <div className="absolute left-1/2 top-1/2 size-28 -translate-x-1/2 -translate-y-1/2 rounded-full border border-white/[0.07]" />
      <div className="absolute left-0 top-1/4 h-1/2 w-[13%] border border-l-0 border-white/[0.07]" />
      <div className="absolute right-0 top-1/4 h-1/2 w-[13%] border border-r-0 border-white/[0.07]" />
    </div>
  )
}

/** Marcadores de um lado (apelido e minutos), só dos golos que o relato já contou. */
function Scorers({ view, said, side, small }: { view: LiveView; said: Line[]; side: 0 | 1; small?: boolean }) {
  const team = side === 0 ? view.home : view.away
  const goals = new Map<string, number[]>()
  // As lesões também, a verde ("Mendoza lesionado 45"), como no CM
  const injuries: [string, number][] = []
  for (const l of said) {
    if (l.side !== side) continue
    const p = [...team.players, ...team.out].find((p) => p.id === l.playerId)
    const name = p ? surname(p.listName) : '?'
    if (isGoalLine(l)) goals.set(name, [...(goals.get(name) ?? []), l.minute])
    else if (l.kind === 'injury' && l.last) injuries.push([name, l.minute])
  }
  return (
    <div className={`flex flex-1 flex-col ${small ? 'gap-0' : 'gap-1'} ${side === 1 ? 'items-start pl-8' : 'items-end pr-8'}`}>
      {[...goals].map(([name, mins]) => (
        <div key={name} className={`flex font-title font-bold ${small ? 'gap-4 text-[17px] leading-[22px]' : 'gap-8 text-[22px]'}`}>
          <span className="text-cm-text">{name}</span>
          <span className="text-cm-yellow">{mins.join(', ')}</span>
        </div>
      ))}
      {injuries.map(([name, minute]) => (
        <div
          key={`${name}-${minute}`}
          className={`flex font-title font-bold ${small ? 'gap-4 text-[17px] leading-[22px]' : 'gap-8 text-[22px]'}`}
        >
          <span className="text-[#4ade80]">{name} lesionado</span>
          <span className="text-cm-yellow">{minute}</span>
        </div>
      ))}
    </div>
  )
}

export function Panorama({ view, line, said, started }: { view: LiveView; line: Line | null; said: Line[]; started: boolean }) {
  // Golos ou lesões: o bloco dos acontecimentos em cima
  const anyGoal = said.some((l) => isGoalLine(l) || (l.kind === 'injury' && l.last))
  const motm = view.finished ? manOfTheMatch(view, said) : null
  const title = !started
    ? 'Pontapé Saída'
    : view.finished
      ? 'Final'
      : view.halfTime
        ? 'Intervalo'
        : view.minute < 45
          ? 'Primeira Parte'
          : 'Segunda Parte'
  const score = view.halfTime
    ? ` · INT ${view.homeGoals}-${view.awayGoals}`
    : view.finished
      ? ` · FIN ${view.homeGoals}-${view.awayGoals}`
      : ''
  // Os cartões que o relato já mostrou
  const cards = (kind: string) => said.filter((l) => l.kind === kind && l.last).length
  const [yellows, reds] = [cards('yellow'), cards('red')]
  return (
    <div className="relative flex h-full flex-col">
      <PitchLines />
      <div className="relative flex h-10 shrink-0 items-center justify-between px-3 text-[13px]">
        <span>
          <span className="font-bold font-mono uppercase tracking-wider text-cm-orange">{title}</span>
          <span className="text-cm-dim"> / </span>
          <span className="text-cm-muted">
            {longDate(view.fixture.date)} {view.fixture.date.slice(0, 4)}
          </span>
        </span>
        <span className="rounded bg-black/50 px-2 py-0.5 text-[12px] font-bold font-mono uppercase tracking-wider text-cm-mint">
          {view.fixture.competition}
          {score}
        </span>
      </div>
      {/* Durante o jogo, os marcadores ficam em cima, longe da caixa do relato */}
      {!(view.halfTime || view.finished) && anyGoal && (
        <div className="relative flex shrink-0 py-1.5">
          <Scorers view={view} said={said} side={0} small />
          <Scorers view={view} said={said} side={1} small />
        </div>
      )}
      <div className="relative flex min-h-0 flex-1 items-center justify-center px-8">
        {view.halfTime || view.finished ? (
          <div className="flex w-full flex-col items-center gap-5">
            <div className="flex w-full">
              <Scorers view={view} said={said} side={0} />
              <Scorers view={view} said={said} side={1} />
            </div>
            {motm && (
              <div className="flex min-w-[360px] flex-col items-center rounded-lg border-2 border-cm-yellow bg-black/70 px-8 py-2">
                <span className="text-[13px] font-bold font-mono uppercase tracking-wider text-cm-yellow">Melhor em Campo</span>
                <span className="font-title text-[28px] font-bold leading-tight text-white">
                  {motm.player.listName.split(', ').reverse().join(' ')}
                </span>
                <span className="flex items-center gap-3 text-[14px]">
                  <span className="rounded px-2 font-bold" style={clubStyle(motm.club)}>
                    {motm.club.shortName}
                  </span>
                  <span className="font-bold text-cm-magenta">Nota {motm.player.rating ?? '-'}</span>
                </span>
              </div>
            )}
          </div>
        ) : (
          // Uma frase nova é um cartão novo (entra com a animação)
          line && <LineCard key={said.length} view={view} line={line} />
        )}
      </div>
      <div className="relative mx-3 mb-2 shrink-0">
        <PossessionBar view={view} home={view.recentPossession} label="Posse · últimos 5 min" />
      </div>
      <div className="relative flex h-8 shrink-0 items-center gap-3 px-3 text-[13px]">
        <span>
          <span className="font-bold text-cm-orange">Árbitro:</span> <span className="text-white">{view.referee}</span>
        </span>
        <span className="text-cm-faint">•</span>
        <span className="flex items-center gap-1.5 text-cm-muted">
          Cartões:
          {yellows + reds === 0 && <span className="text-cm-dim">-</span>}
          {yellows > 0 && <span className="rounded bg-cm-yellow/20 px-1.5 text-[11px] font-bold text-cm-yellow">{yellows} Amr</span>}
          {reds > 0 && <span className="rounded bg-cm-red/20 px-1.5 text-[11px] font-bold text-cm-red">{reds} Verm</span>}
        </span>
        <span className="ml-auto text-cm-orange">
          Tempo: <span className="text-white">{view.weather}</span>
        </span>
        {started && (
          <>
            <span className="text-cm-faint">•</span>
            <span className="text-cm-muted">
              Assistência: <span className="text-white">{view.attendance.toLocaleString('pt-PT')}</span>
            </span>
          </>
        )}
      </div>
    </div>
  )
}
