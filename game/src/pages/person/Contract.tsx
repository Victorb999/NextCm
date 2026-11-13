import type { ReactNode } from 'react'
import type { PersonProfile } from '../../bindings/PersonProfile'
import { Link } from '../../components/Screen'
import { cmDate, money, wage } from '../../format'
import { useGame } from '../../game'
import { useNav } from '../../nav'
import InfoRows from './InfoRows'

/** " (3 anos)": o que falta do contrato, contado a partir da data do jogo. */
function useYearsLeft() {
  const today = useGame().info?.date
  return (until: string) => {
    if (!today) return ''
    const y = Number(until.slice(0, 4)) - Number(today.slice(0, 4)) - (until.slice(5) < today.slice(5) ? 1 : 0)
    return y >= 1 ? ` (${y} ano${y > 1 ? 's' : ''})` : ' (último ano)'
  }
}

export default function Contract({ p }: { p: PersonProfile }) {
  const years = useYearsLeft()
  const nav = useNav()
  const rows: [string, ReactNode][] = [
    [
      'Clube',
      p.club ? (
        <Link key="club" onClick={() => nav.go({ kind: 'club', id: p.club!.id })}>
          {p.club.name}
        </Link>
      ) : (
        'Sem clube'
      ),
    ],
    ['Função', p.job ?? '-'],
    ['No clube desde', cmDate(p.clubJoined)],
    ['Contrato até', p.contractExpires ? `${cmDate(p.contractExpires)}${years(p.contractExpires)}` : '-'],
    ['Salário', wage(p.wage, true)],
    ...(p.player ? ([['Valor', money(p.player.value) || '-']] as [string, ReactNode][]) : []),
    [
      'Nacionalidade',
      p.nation ? (
        <span key="nat">
          {[
            [p.nationId, p.nation],
            [p.secondNationId, p.secondNation],
          ]
            .filter(([, name]) => name)
            .map(([id, name], i) => (
              <span key={i}>
                {i > 0 && ' / '}
                {id !== null ? <Link onClick={() => nav.go({ kind: 'nation', id: id as number })}>{name}</Link> : name}
              </span>
            ))}
        </span>
      ) : (
        '-'
      ),
    ],
  ]
  return <InfoRows rows={rows} />
}
