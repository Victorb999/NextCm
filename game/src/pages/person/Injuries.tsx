import type { ReactNode } from 'react'
import type { PersonProfile } from '../../bindings/PersonProfile'
import { Empty, Panel } from '../../components/Screen'
import { cmDate } from '../../format'
import InfoRows from './InfoRows'

/** Aba "Lesões & Castigos": só num jogo em curso, para jogadores de clubes vivos. */
export default function Injuries({ p }: { p: PersonProfile }) {
  const st = p.status
  if (!st) return <Empty>Sem registos.</Empty>
  const health: [string, ReactNode][] = [
    ['Condição', `${st.condition}%`],
    [
      'Lesão',
      st.injury ? (
        <span key="i" className="text-cm-red">
          {st.injury}
        </span>
      ) : (
        'Nenhuma'
      ),
    ],
    ['Regresso previsto', st.injuryUntil ? cmDate(st.injuryUntil) : '-'],
    ['Moral', st.moraleLabel],
  ]
  const rows: [string, ReactNode][] = [
    [
      'Suspensão',
      st.suspended ? (
        <span key="s" className="text-cm-red">
          {`${st.suspended} jogo${st.suspended > 1 ? 's' : ''}`}
        </span>
      ) : (
        'Nenhuma'
      ),
    ],
    ['Cartões amarelos', `${st.yellows} (suspensão ao 3º)`],
    // Cup-tied: já jogou a continental por outro clube nesta época
    [
      'Continentais',
      st.cupTied.length ? (
        <span key="c" className="text-cm-red">
          {`Não joga: ${st.cupTied.join(', ')}`}
        </span>
      ) : (
        'Pode jogar'
      ),
    ],
  ]
  return (
    <div className="flex min-h-0 flex-1 items-start gap-2 overflow-auto pb-1">
      <Panel title="Condição Física" chip={st.injury ? undefined : 'Apto para jogar'} className="min-w-0 flex-1">
        <InfoRows rows={health} />
      </Panel>
      <Panel title="Disciplina & Suspensões" className="min-w-0 flex-1">
        <InfoRows rows={rows} />
      </Panel>
    </div>
  )
}
