import { useEffect, useState } from 'react'
import { currency, money, moneyStep } from '../format'
import { DialogFrame, DialogRow, Overlay } from './Dialog'

// Diálogo verde do mercado: a proposta a um clube (ou o "Pedir Mais", o preço pedido). O
// valor entra na moeda das Definições; o core recebe sempre euros.

/** Campo de dinheiro com − / +; `value` em euros (multiplicado por `factor` para mostrar). */
function MoneyField({ value, onChange, factor = 1 }: { value: number; onChange: (eur: number) => void; factor?: number }) {
  const c = currency()
  const shown = Math.round(value * factor * c.rate)
  // O que está a ser escrito (enquanto o campo tem o foco)
  const [draft, setDraft] = useState<string | null>(null)
  const set = (v: number) => onChange(Math.max(0, Math.round(v / factor / c.rate)))
  const step = moneyStep(shown)
  return (
    <div className="flex items-center gap-1">
      <button className="cm-purple h-7 w-8" onClick={() => (setDraft(null), set(shown - step))}>
        −
      </button>
      <span className="text-[17px] text-white">{c.symbol}</span>
      <input
        value={draft ?? shown.toLocaleString('pt-BR')}
        onChange={(e) => {
          const digits = e.target.value.replace(/\D/g, '')
          setDraft(digits)
          if (digits) set(Number(digits))
        }}
        onBlur={() => setDraft(null)}
        className="h-7 w-36 border border-[#1d3a1f] bg-black px-2 text-right text-[16px] tabular-nums text-cm-yellow outline-none"
      />
      <button className="cm-purple h-7 w-8" onClick={() => (setDraft(null), set(shown + step))}>
        +
      </button>
    </div>
  )
}

/** Esc cancela e Enter confirma, por cima do resto da tela. */
function useDialogKeys(onCancel: () => void, onOk: () => void) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopImmediatePropagation()
        onCancel()
      }
      if (e.key === 'Enter') {
        e.stopImmediatePropagation()
        onOk()
      }
    }
    window.addEventListener('keydown', onKey, true)
    return () => window.removeEventListener('keydown', onKey, true)
  })
}

/** "Fazer Proposta" (ou "Pedir Mais"): um valor para o clube. */
export function FeeDialog({
  title,
  name,
  value,
  valueLabel = 'Valor',
  feeLabel = 'Proposta',
  initial,
  note,
  error,
  onCancel,
  onOk,
}: {
  title: string
  name: string
  value: number
  valueLabel?: string
  feeLabel?: string
  initial: number
  note?: string
  error?: string | null
  onCancel: () => void
  onOk: (fee: number) => void
}) {
  const [fee, setFee] = useState(initial)
  useDialogKeys(onCancel, () => onOk(fee))
  return (
    <Overlay>
      <DialogFrame title={title} width={560} onCancel={onCancel} onOk={() => onOk(fee)}>
        <DialogRow label="Jogador">
          <span className="text-[17px] text-cm-yellow">{name}</span>
        </DialogRow>
        <DialogRow label={valueLabel}>
          <span className="text-[17px] text-cm-yellow">{money(value)}</span>
        </DialogRow>
        <DialogRow label={feeLabel}>
          <MoneyField value={fee} onChange={setFee} />
        </DialogRow>
        {note && <div className="pt-2 text-[15px] text-white">{note}</div>}
        {error && <div className="pt-2 text-[15px] font-bold text-[#ffb0a0]">{error}</div>}
      </DialogFrame>
    </Overlay>
  )
}
