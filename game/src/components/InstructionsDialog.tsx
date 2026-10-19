import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import type { Instructions } from '../bindings/Instructions'
import type { Mentality } from '../bindings/Mentality'
import type { Passing } from '../bindings/Passing'
import type { Tackling } from '../bindings/Tackling'
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
  onCancel,
  onOk,
}: {
  initial: Instructions
  onCancel: () => void
  onOk: (i: Instructions) => void
}) {
  const [i, setI] = useState(initial)
  const set = (patch: Partial<Instructions>) => setI((old) => ({ ...old, ...patch }))

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopImmediatePropagation()
        onCancel()
      }
      if (e.key === 'Enter') onOk(i)
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
      <DialogFrame title="Instruções à Equipa" width={620} onCancel={onCancel} onOk={() => onOk(i)}>
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
          <div className="pt-1 text-[12px] text-[#bfe6c4]">Durante um jogo, as instruções mudam-se no botão Equipa da tela da partida.</div>
        </div>
      </DialogFrame>
    </Overlay>
  )
}
