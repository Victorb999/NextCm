import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import type { Marking } from '../bindings/Marking'
import type { PassTo } from '../bindings/PassTo'
import type { Passing } from '../bindings/Passing'
import type { PlayerOrders } from '../bindings/PlayerOrders'
import type { SetPiece } from '../bindings/SetPiece'
import type { Tackling } from '../bindings/Tackling'
import { NO_ORDERS } from '../tactics'
import { DialogFrame, DialogRow, Overlay } from './Dialog'

// Instruções individuais de um jogador, como no CM (botão direito num jogador da lista das
// Tácticas): passe, desarme, pressão, passar para, bolas paradas, acções e marcação.

const PASSING: [Passing | null, string][] = [
  [null, 'Equipa'],
  ['mixed', 'Misto'],
  ['short', 'Curto'],
  ['direct', 'Directo'],
  ['long', 'Longo'],
]
const TACKLING: [Tackling | null, string][] = [
  [null, 'Equipa'],
  ['normal', 'Normal'],
  ['soft', 'Suave'],
  ['hard', 'Duro'],
]
const PRESSING: [boolean | null, string][] = [
  [null, 'Equipa'],
  [false, 'Não'],
  [true, 'Sim'],
]
const PASS_TO: [PassTo, string][] = [
  ['mixed', 'D/E/C'],
  ['left', 'Esquerda'],
  ['centre', 'Centro'],
  ['right', 'Direita'],
]
const SET_PIECE: [SetPiece, string][] = [
  ['normal', 'Normal'],
  ['back', 'Atrás'],
  ['forward', 'Frente'],
]
const MARKING: [Marking, string][] = [
  ['zonal', 'Zona'],
  ['none', 'Nenhuma'],
  ['manToMan', 'H-H'],
]
const FLAGS: [keyof PlayerOrders, string][] = [
  ['freeRole', 'Acção Livre'],
  ['forwardRuns', 'Movimentos Frente'],
  ['runWithBall', 'Correr com Bola'],
  ['holdBall', 'Segurar Bola'],
  ['longShots', 'Remates Longe'],
  ['throughBalls', 'Passes Costas'],
  ['crosses', 'Cruzamentos'],
]

function Option({ selected, onClick, children }: { selected: boolean; onClick: () => void; children: ReactNode }) {
  return (
    <button aria-pressed={selected} onClick={onClick} className="cm-option min-w-[70px] py-0">
      {children}
    </button>
  )
}

function Choices<T>({ value, options, onChange }: { value: T; options: [T, string][]; onChange: (v: T) => void }) {
  return (
    <>
      {options.map(([v, label]) => (
        <Option key={label} selected={value === v} onClick={() => onChange(v)}>
          {label}
        </Option>
      ))}
    </>
  )
}

export default function PlayerOrdersDialog({
  title,
  initial,
  onCancel,
  onOk,
}: {
  /** Número e nome do jogador. */
  title: string
  initial: PlayerOrders
  onCancel: () => void
  onOk: (o: PlayerOrders) => void
}) {
  const [o, setO] = useState(initial)
  const set = (patch: Partial<PlayerOrders>) => setO((old) => ({ ...old, ...patch }))

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopImmediatePropagation()
        onCancel()
      }
      if (e.key === 'Enter') onOk(o)
    }
    window.addEventListener('keydown', onKey, true)
    return () => window.removeEventListener('keydown', onKey, true)
  })

  return (
    <Overlay>
      <DialogFrame title={title} width={720} onCancel={onCancel} onOk={() => onOk(o)}>
        <div className="space-y-0.5 py-1">
          <DialogRow label="Passe">
            <Choices value={o.passing} options={PASSING} onChange={(passing) => set({ passing })} />
          </DialogRow>
          <DialogRow label="Desarme">
            <Choices value={o.tackling} options={TACKLING} onChange={(tackling) => set({ tackling })} />
          </DialogRow>
          <DialogRow label="Pressão">
            <Choices value={o.pressing} options={PRESSING} onChange={(pressing) => set({ pressing })} />
          </DialogRow>
          <DialogRow label="Passar para">
            <Choices value={o.passTo} options={PASS_TO} onChange={(passTo) => set({ passTo })} />
          </DialogRow>
          <DialogRow label="Bolas Paradas (A)">
            <Choices value={o.setAttack} options={SET_PIECE} onChange={(setAttack) => set({ setAttack })} />
          </DialogRow>
          <DialogRow label="Bolas Paradas (D)">
            <Choices value={o.setDefence} options={SET_PIECE} onChange={(setDefence) => set({ setDefence })} />
          </DialogRow>
          {FLAGS.map(([key, label]) => (
            <DialogRow key={key} label={label}>
              <Option selected={!o[key]} onClick={() => set({ [key]: false })}>
                Não
              </Option>
              <Option selected={!!o[key]} onClick={() => set({ [key]: true })}>
                Sim
              </Option>
            </DialogRow>
          ))}
          <DialogRow label="Marcação">
            <Choices value={o.marking} options={MARKING} onChange={(marking) => set({ marking })} />
          </DialogRow>
          <div className="flex pt-1">
            <button className="cm-purple h-6 w-40" onClick={() => setO(NO_ORDERS)}>
              Como a Equipa
            </button>
          </div>
        </div>
      </DialogFrame>
    </Overlay>
  )
}
