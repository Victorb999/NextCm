import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { api } from '../api'
import type { ClubRef } from '../bindings/ClubRef'
import type { Fixture } from '../bindings/Fixture'
import type { MatchPreview } from '../bindings/MatchPreview'
import type { PlayerNews } from '../bindings/PlayerNews'
import type { StandingRow } from '../bindings/StandingRow'
import { longDate, matchDate, pensText, shortDate } from '../format'
import { useGame } from '../game'
import { useNav } from '../nav'
import MatchHistory from './match/MatchHistory'
import { Empty, Link, PanelTitle } from './Screen'

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
            {showDate && (
              <td className="w-48 whitespace-nowrap px-2 text-cm-magenta">
                {matchDate(f.date)} <span className="text-[13px] text-cm-muted">{f.time}</span>
              </td>
            )}
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
      className={`block truncate text-[15px] font-semibold ${align} ${c.id === own ? 'text-cm-yellow' : 'text-white'}`}
    >
      {c.shortName}
    </Link>
  )
  // A posição só depois da 1ª jornada (antes fica sem casa)
  const cell = (id: number) => {
    const p = pos(id)
    return p ? (
      <span className="cm-cell w-10 shrink-0 text-center font-mono text-[12px] leading-[20px] text-white">{p}</span>
    ) : (
      <span className="w-10 shrink-0" />
    )
  }
  return (
    <div className="cm-zebra">
      {fixtures.map((f) => (
        <div key={f.id} className="flex h-7 items-center gap-3 px-3">
          {cell(f.home.id)}
          <div className="min-w-0 flex-1">{name(f.home, 'text-right')}</div>
          <span className="w-16 shrink-0 text-center font-mono text-[15px] font-bold leading-none text-cm-magenta">
            {played(f) ? <span className="text-cm-yellow">{`${f.homeGoals} - ${f.awayGoals}`}</span> : 'v'}
            {played(f) && pensText(f) && (
              <span className="block font-sans text-[10px] font-normal text-cm-orange">{pensText(f).trim().slice(1, -1)}</span>
            )}
          </span>
          <div className="min-w-0 flex-1">{name(f.away, 'text-left')}</div>
          {cell(f.away.id)}
        </div>
      ))}
    </div>
  )
}

/** Calendário do clube, como o do CM: data, adversário, C/F, competição e o resultado. */
export function ClubFixtureList({
  clubId,
  fixtures,
  actions,
  selected,
  onSelect,
}: {
  clubId: number
  fixtures: Fixture[]
  /** Botões à esquerda das épocas (Cancelar e Marcar, no clube do treinador). */
  actions?: ReactNode
  /** O jogo escolhido na lista (para o Cancelar). */
  selected?: number | null
  onSelect?: (f: Fixture) => void
}) {
  const nav = useNav()
  if (fixtures.length === 0) return <Empty>Sem jogos marcados.</Empty>
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <PanelTitle
        title={`Temporada ${fixtures[0]?.season ?? ''}`}
        right={
          <>
            {actions}
            <button className="cm-purple h-6 w-28" disabled>
              {'<< Temporada'}
            </button>
            <button className="cm-purple h-6 w-28" disabled>
              {'Temporada >>'}
            </button>
          </>
        }
      />
      <div className="cm-zebra min-h-0 flex-1 overflow-auto">
        {fixtures.map((f) => {
          const home = f.home.id === clubId
          const opponent = home ? f.away : f.home
          return (
            <div
              key={f.id}
              className={`flex h-6 items-center gap-3 px-3 ${onSelect ? 'cursor-pointer' : ''} ${f.id === selected ? '!bg-cm-sel' : ''}`}
              onMouseDown={onSelect ? () => onSelect(f) : undefined}
            >
              <span className="cm-cell w-28 shrink-0 text-center text-[13px] text-white">{shortDate(f.date)}</span>
              <span className="w-11 shrink-0 text-center text-[12.5px] text-cm-muted">{f.time}</span>
              <div className="min-w-0 flex-1 truncate">
                <ClubName club={opponent} className="!text-[16px] font-semibold" />
              </div>
              <span className="w-6 shrink-0 text-center font-bold text-cm-magenta">{home ? 'C' : 'F'}</span>
              <span className="w-52 shrink-0 truncate text-cm-orange" title={f.label}>
                {f.competition}
              </span>
              {played(f) ? (
                // O resultado abre a ficha do jogo (a das reservas também)
                <button
                  className="cm-cell w-16 shrink-0 text-center tabular-nums text-white hover:text-cm-yellow"
                  title={pensText(f).trim() || 'Ver o jogo'}
                  onClick={() => nav.go({ kind: 'match', id: f.id })}
                >
                  {`${f.homeGoals}:${f.awayGoals}`}
                </button>
              ) : (
                <span className="cm-cell w-16 shrink-0 text-center tabular-nums text-white">---</span>
              )}
            </div>
          )
        })}
      </div>
    </div>
  )
}

function InfoRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex min-h-6 items-baseline gap-4 px-3 py-1">
      <span className="w-56 shrink-0 text-[15px] font-semibold text-white">{label}</span>
      <div className="flex-1 text-[14px] text-cm-yellow">{children}</div>
    </div>
  )
}

/** As ausências de um lado: o jogador (link para a ficha) e o motivo. */
function News({ list }: { list: PlayerNews[] | undefined }) {
  const nav = useNav()
  if (!list) return null
  if (list.length === 0) return <>Elenco todo disponível</>
  return (
    <>
      {list.map((n) => (
        <div key={n.id}>
          <Link onClick={() => nav.go({ kind: 'person', id: n.id })}>{n.name}</Link> {n.text}
        </div>
      ))}
    </>
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
  const nav = useNav()
  const [index, setIndex] = useState(start)
  const [preview, setPreview] = useState<MatchPreview | null>(null)
  const fixture = fixtures[index]

  useEffect(() => {
    if (fixture && !played(fixture)) api.matchPreview(fixture.id).then(setPreview, () => setPreview(null))
  }, [fixture])

  if (!fixture) return <Empty>{empty}</Empty>
  const p = preview?.fixture.id === fixture.id ? preview : null

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <PanelTitle
        title={
          <>
            <span className="flex items-center gap-1.5 normal-case tracking-normal">
              <button className="cm-purple h-6 w-28" disabled={index <= 0} onClick={() => setIndex(index - 1)}>
                {'<< Data'}
              </button>
              <button className="cm-purple h-6 w-28" disabled={index >= fixtures.length - 1} onClick={() => setIndex(index + 1)}>
                {'Data >>'}
              </button>
            </span>
            <span className="ml-2">
              <Link onClick={() => nav.go({ kind: 'competition', id: fixture.competitionId })}>{fixture.competition}</Link>
              {` · ${fixture.label}`}
            </span>
          </>
        }
      />
      {/* Um jogo já disputado: a partida inteira, como no CM */}
      {played(fixture) ? (
        <div className="flex min-h-0 flex-1 flex-col p-2">
          <MatchHistory id={fixture.id} />
        </div>
      ) : (
        <MatchPreviewRows fixture={fixture} clubId={clubId} preview={p} />
      )}
    </div>
  )
}

/** O Próximo Jogo: data, local, regras, último confronto, odds, tempo e as notícias dos dois clubes. */
function MatchPreviewRows({ fixture, clubId, preview: p }: { fixture: Fixture; clubId: number; preview: MatchPreview | null }) {
  const game = useGame()
  const home = fixture.home.id === clubId
  const us = home ? fixture.home : fixture.away
  const them = home ? fixture.away : fixture.home
  const last = p?.lastMeeting
  return (
    <>
      <div className="shrink-0 py-2 text-center font-title text-[22px] font-bold text-cm-yellow">
        <ClubName club={them} className="!text-[22px] !text-cm-yellow" /> ({fixture.neutral ? 'Neutro' : home ? 'Casa' : 'Fora'})
      </div>
      <div className="min-h-0 flex-1 overflow-auto pb-2">
        <div className="cm-zebra">
          {played(fixture) && (
            <InfoRow label="Resultado">
              <ClubName club={fixture.home} className="!text-[14px] !text-cm-yellow" /> {fixture.homeGoals}-{fixture.awayGoals}{' '}
              <ClubName club={fixture.away} className="!text-[14px] !text-cm-yellow" />
              {pensText(fixture)}
            </InfoRow>
          )}
          <InfoRow label="Data">{`${longDate(fixture.date, game.info?.date)}, ${fixture.time}`}</InfoRow>
          <InfoRow label="Local">{p ? [p.venue, p.city].filter(Boolean).join(', ') || '-' : '…'}</InfoRow>
          <InfoRow label="Regras do Jogo">
            <div>Não há restrições sobre jogadores</div>
            <div>9 suplentes convocados, máximo 5 utilizados</div>
          </InfoRow>
          <InfoRow label="Último Confronto">
            {last
              ? `${last.home.shortName} ${last.homeGoals}-${last.awayGoals} ${last.away.shortName}, ${last.competition} ${shortDate(last.date)}`
              : 'Nenhum nesta temporada'}
          </InfoRow>
          {p?.odds && (
            <InfoRow label="Odds">
              {`${fixture.home.shortName} ${p.odds[0].toFixed(2)} · Empate ${p.odds[1].toFixed(2)} · ${fixture.away.shortName} ${p.odds[2].toFixed(2)}`}
            </InfoRow>
          )}
          <InfoRow label="Tempo">{p?.weather ?? '…'}</InfoRow>
          <InfoRow label="Condições do Gramado">{p?.pitch ?? '…'}</InfoRow>
          <InfoRow label={`Notícias: ${us.shortName}`}>
            <News list={home ? p?.homeNews : p?.awayNews} />
          </InfoRow>
          <InfoRow label={`Notícias: ${them.shortName}`}>
            <News list={home ? p?.awayNews : p?.homeNews} />
          </InfoRow>
        </div>
      </div>
    </>
  )
}
