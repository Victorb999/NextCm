import { useEffect, useState } from 'react'
import { api } from '../api'
import type { NewsSubRow } from '../bindings/NewsSubRow'
import type { NewsSubs } from '../bindings/NewsSubs'
import { DialogFrame, OptionGroup, OptionLine, Overlay } from './Dialog'

// Assinaturas das notícias (como as preferências da caixa de entrada do FM): em cada
// competição, os resultados, as lesões graves e as transferências; a Lista Preferencial; e
// os temas que param o Continuar Jogo. Cada clique grava no core.

const COLUMNS: { key: 'results' | 'injuries' | 'transfers'; label: string }[] = [
  { key: 'results', label: 'Resultados' },
  { key: 'injuries', label: 'Lesões Graves' },
  { key: 'transfers', label: 'Transferências' },
]

const STOPS: [string, string][] = [
  ['results', 'Resultados de impacto'],
  ['injuries', 'Lesões graves'],
  ['transfers', 'Transferências do dia'],
  ['shortlist', 'Lista Preferencial'],
]

/** Os grupos da grelha: as competições do treinador, depois as continentais e cada nação. */
function groups(rows: NewsSubRow[]): [string, NewsSubRow[]][] {
  const out: [string, NewsSubRow[]][] = []
  for (const r of rows) {
    const title = r.own ? 'As suas competições' : (r.nation ?? 'Internacionais')
    const last = out[out.length - 1]
    if (last && last[0] === title) last[1].push(r)
    else out.push([title, [r]])
  }
  return out
}

function Cell({ on, onToggle }: { on: boolean | null; onToggle: () => void }) {
  if (on === null) return <span className="w-32 text-center text-cm-dim">-</span>
  return (
    <button
      role="checkbox"
      aria-checked={on}
      onClick={onToggle}
      className={`w-32 text-center font-title text-[17px] hover:underline ${on ? 'font-bold text-white' : 'text-cm-purplelight'}`}
    >
      {on ? 'Sim' : 'Não'}
    </button>
  )
}

export default function NewsSubsDialog({ onClose }: { onClose: () => void }) {
  const [subs, setSubs] = useState<NewsSubs | null>(null)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    api.newsSubs().then(setSubs, (e) => setError(String(e)))
  }, [])
  const set = (competitionId: number, topic: string, on: boolean) => {
    api
      .setNewsSub(competitionId, topic, on)
      .then(() => api.newsSubs())
      .then(setSubs, (e) => setError(String(e)))
  }
  return (
    <Overlay>
      <DialogFrame
        title="Assinaturas de Notícias"
        width={780}
        onClose={onClose}
        footer={
          <button className="cm-purple h-9 flex-1 font-title text-xl text-cm-yellow" onClick={onClose}>
            Ok
          </button>
        }
      >
        {error && <div className="pb-2 text-cm-red">{error}</div>}
        {subs && (
          <>
            {groups(subs.rows).map(([title, rows]) => (
              <OptionGroup key={title} title={title}>
                <div className="flex h-[26px] items-center px-3 text-[14px] text-cm-yellow">
                  <span className="flex-1">Competição</span>
                  {COLUMNS.map((c) => (
                    <span key={c.key} className="w-32 text-center">
                      {c.label}
                    </span>
                  ))}
                </div>
                {rows.map((r) => (
                  <div key={r.competitionId} className="flex h-[26px] items-center px-3">
                    <span className="flex-1 truncate font-title text-[17px] text-white">{r.name}</span>
                    {COLUMNS.map((c) => {
                      const on = r[c.key]
                      return <Cell key={c.key} on={on} onToggle={() => set(r.competitionId, c.key, !on)} />
                    })}
                  </div>
                ))}
              </OptionGroup>
            ))}
            <OptionGroup title="Lista Preferencial">
              <OptionLine
                label="Lesões, suspensões e transferências dos jogadores da Lista Preferencial"
                on={subs.shortlist}
                onToggle={() => set(0, 'shortlist', !subs.shortlist)}
              />
            </OptionGroup>
            <OptionGroup title="Parar o Continuar Jogo">
              {STOPS.map(([key, label]) => {
                const on = subs.stop.includes(key)
                return <OptionLine key={key} label={label} on={on} onToggle={() => set(-1, key, !on)} />
              })}
            </OptionGroup>
          </>
        )}
      </DialogFrame>
    </Overlay>
  )
}
