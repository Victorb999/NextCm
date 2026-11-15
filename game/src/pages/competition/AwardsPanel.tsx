import { useState } from 'react'
import { api } from '../../api'
import type { AwardLine } from '../../bindings/AwardLine'
import { Empty, ErrorBox, Link, Panel } from '../../components/Screen'
import { useNav } from '../../nav'
import { useLoad } from '../../useLoad'

// A aba Prêmios da competição: os da temporada (Melhor Jogador, Artilheiro, Time da
// Temporada...) e os do mês, de cada temporada, das mais recentes para as mais antigas.

const MONTHS = ['Janeiro', 'Fevereiro', 'Março', 'Abril', 'Maio', 'Junho', 'Julho', 'Agosto', 'Setembro', 'Outubro', 'Novembro', 'Dezembro']

/** O mês premiado: o prémio do mês sai no dia 1 do mês seguinte. */
function awardedMonth(date: string) {
  const month = Number(date.slice(5, 7))
  return MONTHS[(month + 10) % 12] ?? ''
}

/** O valor de um prémio: a nota média, os gols ou os pontos do treinador. */
function valueText(a: AwardLine) {
  if (a.value === null) return ''
  if (a.kind === 'top_scorer') return `${a.value} gols`
  if (a.kind.startsWith('manager')) return ''
  return a.value.toFixed(2).replace('.', ',')
}

function Line({ a, label }: { a: AwardLine; label: string }) {
  const nav = useNav()
  return (
    <div className="flex h-7 items-center gap-3 px-3 text-[14px]">
      <span className="w-40 shrink-0 text-cm-text">{label}</span>
      <span className="min-w-0 flex-1 truncate font-semibold text-white">
        <Link onClick={() => nav.go({ kind: 'person', id: a.personId })}>{a.name}</Link>
      </span>
      <span className="w-40 shrink-0 truncate text-cm-lilac">
        {a.club && <Link onClick={() => a.club && nav.go({ kind: 'club', id: a.club.id })}>{a.club.shortName}</Link>}
      </span>
      <span className="w-20 shrink-0 text-right font-mono text-cm-yellow">{valueText(a)}</span>
    </div>
  )
}

export default function AwardsPanel({ id }: { id: number }) {
  const [awards, setAwards] = useState<AwardLine[] | null>(null)
  const [error, setError] = useState<string | null>(null)
  useLoad(() => api.competitionAwards(id), [id], setAwards, setError)
  if (error) return <ErrorBox>{error}</ErrorBox>
  if (!awards) return null
  if (!awards.length)
    return <Empty>Ainda não há prêmios nesta competição: os do mês saem no dia 1 e os da temporada quando ela acaba.</Empty>
  const seasons = [...new Set(awards.map((a) => a.season))]
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2 overflow-auto">
      {seasons.map((season) => {
        const list = awards.filter((a) => a.season === season)
        const year = list.filter((a) => !a.monthly && a.kind !== 'team_season')
        const team = list.filter((a) => a.kind === 'team_season')
        const months = [...new Set(list.filter((a) => a.monthly).map((a) => a.date))]
        return (
          <Panel key={season} title={`Temporada ${season}`} className="shrink-0">
            {year.length > 0 && (
              <div className="cm-zebra">
                {year.map((a) => (
                  <Line key={`${a.kind}${a.personId}`} a={a} label={a.label} />
                ))}
              </div>
            )}
            {team.length > 0 && (
              <>
                <div className="mt-1 px-3 pt-1 text-[12px] font-bold uppercase tracking-wide text-cm-orange">Time da Temporada</div>
                <div className="cm-zebra">
                  {team.map((a) => (
                    <Line key={`t${a.personId}`} a={a} label="" />
                  ))}
                </div>
              </>
            )}
            {months.map((date) => (
              <div key={date}>
                <div className="mt-1 px-3 pt-1 text-[12px] font-bold uppercase tracking-wide text-cm-orange">{awardedMonth(date)}</div>
                <div className="cm-zebra">
                  {list
                    .filter((a) => a.monthly && a.date === date)
                    .map((a) => (
                      <Line key={`${a.kind}${a.personId}`} a={a} label={a.label} />
                    ))}
                </div>
              </div>
            ))}
          </Panel>
        )
      })}
    </div>
  )
}
