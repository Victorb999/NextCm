import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import type { Instructions } from '../bindings/Instructions'
import type { Mentality } from '../bindings/Mentality'
import type { Passing } from '../bindings/Passing'
import type { Tackling } from '../bindings/Tackling'
import type { Takers } from '../bindings/Takers'
import { DialogFrame, DialogRow, Overlay } from './Dialog'

const MENTALITY: [Mentality, string][] = [
  ['normal', 'Normal'],
  ['defensive', 'Defensivo'],
  ['attacking', 'Atacante'],
]
const PASSING: [Passing, string][] = [
  ['mixed', 'Misto'],
  ['short', 'Curto'],
  ['direct', 'Directo'],
  ['long', 'Longo'],
]
const TACKLING: [Tackling, string][] = [
  ['normal', 'Normal'],
  ['soft', 'Suave'],
  ['hard', 'Duro'],
]

function Option({ selected, onClick, children }: { selected: boolean; onClick: () => void; children: ReactNode }) {
  return (
    <button aria-pressed={selected} onClick={onClick} className="cm-option min-w-[78px] py-0.5">
      {children}
    </button>
  )
}

/** "Instruções à Equipa", o diálogo verde da tela de Tácticas do CM. */
export default function InstructionsDialog({
  initial,
  takers: initialTakers,
  players,
  onCancel,
  onOk,
}: {
  initial: Instructions
  takers: Takers
  /** Os titulares, para escolher os batedores e o organizador. */
  players: { id: number; name: string }[]
  onCancel: () => void
  onOk: (i: Instructions, takers: Takers) => void
}) {
  const [i, setI] = useState(initial)
  const [k, setK] = useState(initialTakers)
  const set = (patch: Partial<Instructions>) => setI((old) => ({ ...old, ...patch }))

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopImmediatePropagation()
        onCancel()
      }
      if (e.key === 'Enter') onOk(i, k)
    }
    window.addEventListener('keydown', onKey, true)
    return () => window.removeEventListener('keydown', onKey, true)
  })

  const yesNo = (label: string, key: 'pressing' | 'offsideTrap' | 'counterAttack' | 'tightMarking') => (
    <DialogRow label={label}>
      <Option selected={!i[key]} onClick={() => set({ [key]: false })}>
        Não
      </Option>
      <Option selected={i[key]} onClick={() => set({ [key]: true })}>
        Sim
      </Option>
    </DialogRow>
  )

  return (
    <Overlay>
      <DialogFrame title="Instruções à Equipa" width={620} onCancel={onCancel} onOk={() => onOk(i, k)}>
        <div className="space-y-1 py-1">
          <DialogRow label="Mentalidade">
            {MENTALITY.map(([k, label]) => (
              <Option key={k} selected={i.mentality === k} onClick={() => set({ mentality: k })}>
                {label}
              </Option>
            ))}
          </DialogRow>
          <DialogRow label="Passe">
            {PASSING.map(([k, label]) => (
              <Option key={k} selected={i.passing === k} onClick={() => set({ passing: k })}>
                {label}
              </Option>
            ))}
          </DialogRow>
          <DialogRow label="Desarme">
            {TACKLING.map(([k, label]) => (
              <Option key={k} selected={i.tackling === k} onClick={() => set({ tackling: k })}>
                {label}
              </Option>
            ))}
          </DialogRow>
          {yesNo('Pressão', 'pressing')}
          {yesNo('Defesa em Linha', 'offsideTrap')}
          {yesNo('Contra-Ataque', 'counterAttack')}
          {yesNo('Defesa Intensa', 'tightMarking')}
          {(
            [
              ['penalty', 'Penáltis'],
              ['freeKick', 'Livres'],
              ['corner', 'Cantos'],
              ['playmaker', 'Organizador'],
              ['captain', 'Capitão'],
            ] as [keyof Takers, string][]
          ).map(([key, label]) => (
            <DialogRow key={key} label={label}>
              <select
                value={k[key] ?? ''}
                onChange={(e) => setK((old) => ({ ...old, [key]: e.target.value ? Number(e.target.value) : null }))}
                className="cm-field min-w-56"
              >
                <option value="">Adjunto escolhe</option>
                {players.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
              </select>
            </DialogRow>
          ))}
          <div className="pt-1 text-[12px] text-[#bfe6c4]">Durante um jogo, as instruções mudam-se no botão Equipa da tela da partida.</div>
        </div>
      </DialogFrame>
    </Overlay>
  )
}
