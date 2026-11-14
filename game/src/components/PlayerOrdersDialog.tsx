import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { api } from '../api'
import { PLAYER_ATTRIBUTES, valueColor } from '../attributes'
import type { PlayerOrders } from '../bindings/PlayerOrders'
import {
  NO_ORDERS,
  ORDER_FLAGS as FLAGS,
  ORDER_MARKING as MARKING,
  ORDER_PASSING as PASSING,
  ORDER_PASS_TO as PASS_TO,
  ORDER_PRESSING as PRESSING,
  ORDER_SET_PIECE as SET_PIECE,
  ORDER_TACKLING as TACKLING,
} from '../tactics'
import { DialogFrame, DialogRow, Overlay } from './Dialog'

// Instruções individuais de um jogador, como no CM (botão direito num jogador da lista das
// Tácticas): passe, desarme, pressão, passar para, bolas paradas, acções e marcação. Ao lado, os
// atributos do jogador (o que o clube sabe), para não ter de ir ao perfil e voltar.

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

/** Os atributos do jogador em duas colunas, com as cores do perfil. */
function Attributes({ clubId, personId }: { clubId: number; personId: number }) {
  const [attrs, setAttrs] = useState<Record<string, number | null> | null>(null)
  useEffect(() => {
    api.squadAttributes(clubId).then(
      (rows) => setAttrs(rows.find((r) => r.person === personId)?.attributes ?? {}),
      () => setAttrs({}),
    )
  }, [clubId, personId])
  const half = Math.ceil(PLAYER_ATTRIBUTES.length / 2)
  const columns = [PLAYER_ATTRIBUTES.slice(0, half), PLAYER_ATTRIBUTES.slice(half)]
  return (
    <div className="grid w-[380px] shrink-0 grid-cols-2 gap-1.5 py-1">
      {columns.map((col, c) => (
        <div key={c} className="cm-zebra overflow-hidden rounded [&>*]:px-2">
          {col.map(([key, label]) => {
            const v = attrs?.[key] ?? null
            return (
              <div key={key} className="flex h-[26px] items-center justify-between text-[13.5px]">
                <span className="truncate text-cm-text">{label}</span>
                <span className={`font-bold tabular-nums ${v ? valueColor(v) : 'text-[#777]'}`}>{attrs ? (v ?? '-') : ''}</span>
              </div>
            )
          })}
        </div>
      ))}
    </div>
  )
}

export default function PlayerOrdersDialog({
  title,
  clubId,
  personId,
  initial,
  onCancel,
  onOk,
}: {
  /** Número e nome do jogador. */
  title: string
  /** O clube e o jogador: os atributos ao lado das instruções. */
  clubId: number
  personId: number
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
      <DialogFrame title={title} width={1110} onCancel={onCancel} onOk={() => onOk(o)}>
        <div className="flex gap-3">
          <div className="min-w-0 flex-1">
            {/* Linhas em zebra, como as listas */}
            <div className="cm-zebra py-1 [&>*]:rounded [&>*]:px-2">
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
            </div>
            <div className="flex px-2 pb-1 pt-1">
              <button className="cm-purple h-6 w-40" onClick={() => setO(NO_ORDERS)}>
                Como a Equipa
              </button>
            </div>
          </div>
          <Attributes clubId={clubId} personId={personId} />
        </div>
      </DialogFrame>
    </Overlay>
  )
}
