import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { api } from '../api'
import type { ClubRef } from '../bindings/ClubRef'
import type { Fixture } from '../bindings/Fixture'
import type { MatchPreview } from '../bindings/MatchPreview'
import type { StandingRow } from '../bindings/StandingRow'
import { longDate, matchDate, pensText, shortDate } from '../format'
import { useGame } from '../game'
import { useNav } from '../nav'
import { Link } from './Screen'

// Listas de jogos, o calendário do clube e o Próximo/Último Jogo, no estilo das telas do CM.

const played = (f: Fixture) => f.homeGoals !== null && f.awayGoals !== null

function ClubName({ club, own, className = '' }: { club: ClubRef; own?: number; className?: string }) {
  const nav = useNav()
  return (
    <Link
      onClick={() => nav.go({ kind: 'club', id: club.id })}
      className={`font-title text-[17px] ${club.id === own ? 'text-cm-pink' : 'text-white'} ${className}`}
    >
      {club.shortName}
    </Link>
  )
}

/** Jogos de uma jornada ou de um dia: casa  2 - 1  fora. */
export function FixtureRows({ fixtures, own, showDate = true }: { fixtures: Fixture[]; own?: number; showDate?: boolean }) {
  return (
    <table className="w-full border-collapse">
      <tbody>
        {fixtures.map((f) => (
          <tr key={f.id} className="h-[30px] text-[15px]">
            {showDate && <td className="w-36 whitespace-nowrap px-2 text-cm-magenta">{matchDate(f.date)}</td>}
            <td className="px-2 text-right">
              <ClubName club={f.home} own={own} />
            </td>
            <td className="w-36 whitespace-nowrap text-center font-bold tabular-nums text-cm-yellow">
              {played(f) ? `${f.homeGoals} - ${f.awayGoals}` : 'v'}
              {played(f) && <span className="text-[12px] font-normal text-cm-orange">{pensText(f)}</span>}
            </td>
            <td className="px-2">
              <ClubName club={f.away} own={own} />
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  )
}

/**
 * Jogos de uma jornada como "Jogos da Tarde" do CM: a posição de cada clube numa casa,
 * os nomes grandes (o do treinador em amarelo) e o "v" (ou o resultado) no meio.
 */
export function RoundGames({ fixtures, table, own }: { fixtures: Fixture[]; table: StandingRow[]; own?: number }) {
  const nav = useNav()
  const pos = (id: number) => {
    const r = table.find((r) => r.club.id === id)
    return r && r.played > 0 ? `${r.position}º` : ''
  }
  const name = (c: ClubRef, align: string) => (
    <Link
      onClick={() => nav.go({ kind: 'club', id: c.id })}
      className={`block truncate font-title text-[21px] leading-[28px] ${align} ${c.id === own ? 'text-cm-yellow' : 'text-white'}`}
    >
      {c.shortName}
    </Link>
  )
  return (
    <div className="space-y-0.5">
      {fixtures.map((f) => (
        <div key={f.id} className="flex h-[30px] items-center gap-3 px-2">
          <span className="cm-cell h-[22px] w-11 shrink-0 text-center text-[13px] leading-[20px] text-white">{pos(f.home.id)}</span>
          <div className="min-w-0 flex-1">{name(f.home, 'text-left')}</div>
          <span className="w-16 shrink-0 text-center font-title text-[20px] font-bold leading-none text-cm-magenta">
            {played(f) ? <span className="text-cm-yellow">{`${f.homeGoals} - ${f.awayGoals}`}</span> : 'v'}
            {played(f) && f.homePens !== null && (
              <span className="block font-sans text-[11px] font-normal text-cm-orange">{`${f.homePens}-${f.awayPens} g.p.`}</span>
            )}
          </span>
          <div className="min-w-0 flex-1">{name(f.away, 'text-left')}</div>
          <span className="cm-cell h-[22px] w-11 shrink-0 text-center text-[13px] leading-[20px] text-white">{pos(f.away.id)}</span>
        </div>
      ))}
    </div>
  )
}


/** Calendário do clube, como o do CM: data, adversário, C/F, competição e o resultado. */
export function ClubFixtureList({ clubId, fixtures }: { clubId: number; fixtures: Fixture[] }) {
  if (fixtures.length === 0) return <div className="p-6 text-center text-cm-magenta">Sem jogos marcados.</div>
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex shrink-0 items-center gap-1 px-1.5 pt-1.5">
        <button className="cm-purple h-6 w-36" disabled>
          {'<< Época'}
        </button>
        <button className="cm-purple h-6 w-36" disabled>
          {'Época >>'}
        </button>
      </div>
      <div className="shrink-0 py-1.5 text-center font-title text-xl font-bold text-cm-yellow">Época {fixtures[0].season}</div>
      <div className="min-h-0 flex-1 overflow-auto px-2 pb-2">
        {fixtures.map((f) => {
          const home = f.home.id === clubId
          const opponent = home ? f.away : f.home
          return (
            <div key={f.id} className="flex h-[25px] items-center gap-3">
              <span className="cm-cell w-28 shrink-0 text-center text-[13px] text-white">{shortDate(f.date)}</span>
              <div className="min-w-0 flex-1 truncate">
                <ClubName club={opponent} className="!text-[20px] leading-[26px]" />
              </div>
              <span className="w-6 shrink-0 text-center font-bold text-cm-magenta">{home ? 'C' : 'F'}</span>
              <span className="w-52 shrink-0 truncate text-cm-orange" title={f.label}>
                {f.competition}
              </span>
              <span className="cm-cell w-16 shrink-0 text-center tabular-nums text-white" title={pensText(f).trim() || undefined}>
                {played(f) ? `${f.homeGoals}:${f.awayGoals}` : '---'}
              </span>
            </div>
          )
        })}
      </div>
    </div>
  )
}

function InfoRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex min-h-[24px] items-baseline gap-4">
      <span className="w-60 shrink-0 font-title text-[20px] leading-tight text-white">{label}</span>
      <div className="flex-1 text-[15px] text-cm-yellow">{children}</div>
    </div>
  )
}

/**
 * Próximo Jogo / Último Jogo do CM: "Adversário (Casa)", a competição e a ficha do jogo
 * (data, local, regras, último confronto, tempo, terreno e as notícias de cada lado).
 * "<< Data" e "Data >>" andam pelos jogos do clube.
 */
export function MatchInfo({
  clubId,
  fixtures,
  start,
  empty,
}: {
  clubId: number
  fixtures: Fixture[]
  /** Índice inicial em `fixtures` (o próximo jogo ou o último). */
  start: number
  empty: string
}) {
  const game = useGame()
  const [index, setIndex] = useState(start)
  const [preview, setPreview] = useState<MatchPreview | null>(null)
  const fixture = fixtures[index]

  useEffect(() => {
    if (fixture) api.matchPreview(fixture.id).then(setPreview, () => setPreview(null))
  }, [fixture])

  if (!fixture) return <div className="p-6 text-center text-cm-magenta">{empty}</div>
  const home = fixture.home.id === clubId
  const us = home ? fixture.home : fixture.away
  const them = home ? fixture.away : fixture.home
  const p = preview?.fixture.id === fixture.id ? preview : null
  const last = p?.lastMeeting

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex shrink-0 items-center gap-1 px-1.5 pt-1.5">
        <button className="cm-purple h-6 w-36" disabled={index <= 0} onClick={() => setIndex(index - 1)}>
          {'<< Data'}
        </button>
        <button className="cm-purple h-6 w-36" disabled={index >= fixtures.length - 1} onClick={() => setIndex(index + 1)}>
          {'Data >>'}
        </button>
      </div>
      <div className="shrink-0 py-1.5 text-center font-title text-[22px] font-bold text-cm-yellow">
        <ClubName club={them} className="!text-[22px] !text-cm-yellow" /> ({home ? 'Casa' : 'Fora'})
      </div>
      <div className="min-h-0 flex-1 overflow-auto px-4 pb-2">
        <div className="mb-2 inline-block border-b-2 border-cm-orange font-title text-[20px] font-bold text-cm-orange">
          {fixture.competition} · {fixture.label}
        </div>
        <div className="space-y-0.5">
          {played(fixture) && (
            <InfoRow label="Resultado">
              {fixture.home.shortName} {fixture.homeGoals}-{fixture.awayGoals} {fixture.away.shortName}
              {pensText(fixture)}
            </InfoRow>
          )}
          <InfoRow label="Data">{longDate(fixture.date, game.info?.date)}</InfoRow>
          <InfoRow label="Local">{p ? [p.venue, p.city].filter(Boolean).join(', ') || '-' : '…'}</InfoRow>
          <InfoRow label="Regras do Jogo">
            <div>Não há restrições sobre jogadores</div>
            <div>9 suplentes convocados, máximo 5 utilizados</div>
          </InfoRow>
          <InfoRow label="Último Confronto">
            {last
              ? `${last.home.shortName} ${last.homeGoals}-${last.awayGoals} ${last.away.shortName}, ${last.competition} ${shortDate(last.date)}`
              : 'Nenhum nesta época'}
          </InfoRow>
          <InfoRow label="Tempo">{p?.weather ?? '…'}</InfoRow>
          <InfoRow label="Condições Terreno Jogo">{p?.pitch ?? '…'}</InfoRow>
          <InfoRow label={`Notícias: ${us.shortName}`}>
            {(home ? p?.homeNews : p?.awayNews)?.map((n) => <div key={n}>{n}</div>) ?? null}
            {p && (home ? p.homeNews : p.awayNews).length === 0 && 'Plantel todo disponível'}
          </InfoRow>
          <InfoRow label={`Notícias: ${them.shortName}`}>
            {(home ? p?.awayNews : p?.homeNews)?.map((n) => <div key={n}>{n}</div>) ?? null}
            {p && (home ? p.awayNews : p.homeNews).length === 0 && 'Plantel todo disponível'}
          </InfoRow>
        </div>
      </div>
    </div>
  )
}
