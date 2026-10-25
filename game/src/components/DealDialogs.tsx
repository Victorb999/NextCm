import { useEffect, useState } from 'react'
import type { DealTerms } from '../bindings/DealTerms'
import type { LoanTerms } from '../bindings/LoanTerms'
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

/** − valor + a percorrer uma lista de opções (prestações, percentagens, durações). */
function Choice<T>({ value, options, onChange }: { value: T; options: [T, string][]; onChange: (v: T) => void }) {
  const i = Math.max(
    0,
    options.findIndex((o) => o[0] === value),
  )
  return (
    <div className="flex items-center gap-1">
      <button className="cm-purple h-7 w-8" disabled={i === 0} onClick={() => onChange(options[i - 1][0])}>
        −
      </button>
      <span className="w-36 text-center text-[17px] text-cm-yellow">{options[i][1]}</span>
      <button className="cm-purple h-7 w-8" disabled={i === options.length - 1} onClick={() => onChange(options[i + 1][0])}>
        +
      </button>
    </div>
  )
}

const INSTALMENTS: [number, string][] = [
  [0, '-'],
  [6, '6 meses'],
  [12, '12 meses'],
  [24, '24 meses'],
  [36, '36 meses'],
]
const GAMES: [number, string][] = [10, 20, 30, 40, 50].map((n) => [n, `${n} jogos`])
// Os valores da lista amarela do CM
const PRESETS = [0, 100_000, 500_000, 1_000_000, 5_000_000, 15_000_000, 30_000_000, 50_000_000, 75_000_000, 100_000_000, 125_000_000, 150_000_000]

/** O "▾" do CM: escolher o valor numa lista amarela. */
function Presets({ onPick }: { onPick: (eur: number) => void }) {
  const [open, setOpen] = useState(false)
  return (
    <div className="relative">
      <button className="cm-purple h-7 w-6" onClick={() => setOpen((o) => !o)}>
        ▾
      </button>
      {open && (
        <div className="absolute left-0 top-7 z-10 min-w-28 border border-[#7a5a08] bg-[#e7b51f] text-[14px] text-[#1c1400]">
          {PRESETS.map((v) => (
            <div
              key={v}
              className="cursor-pointer px-2 hover:bg-[#ffe680]"
              onClick={() => {
                onPick(v)
                setOpen(false)
              }}
            >
              {money(v) === '-' ? `${currency().symbol}0` : money(v)}
            </div>
          ))}
        </div>
      )}
    </div>
  )
}

const SELL_ON: [number, string][] = Array.from({ length: 11 }, (_, i) => [i * 5, i ? `${i * 5}%` : '-'])
const LOAN_LENGTH: [number, string][] = [
  [0, 'Fim da época'],
  [6, '6 meses'],
  [12, '1 ano'],
]
const WAGE_SHARE: [number, string][] = [0, 25, 50, 75, 100].map((v) => [v, `${v}%`])

/** "Proposta de transferência" do CM: valor, prestações mensais, percentagem da próxima
 *  venda e um valor depois de 10 jogos. */
export function TransferOfferDialog({
  title,
  name,
  club,
  value,
  budgetText,
  initial,
  squad,
  okLabel = 'Oferecer',
  error,
  onCancel,
  onOk,
}: {
  title?: string
  name: string
  club: string
  value: number
  budgetText: string
  initial: DealTerms
  /** Jogadores do plantel que podem ir na troca; sem lista, não há troca nem proposta pública ("Pedir Mais"). */
  squad?: { id: number; name: string }[]
  okLabel?: string
  error?: string | null
  onCancel: () => void
  onOk: (t: DealTerms) => void
}) {
  const [t, setT] = useState(initial)
  const set = (p: Partial<DealTerms>) => setT((old) => ({ ...old, ...p }))
  const [picking, setPicking] = useState(false)
  useDialogKeys(onCancel, () => onOk(t))
  const exchange = squad?.find((p) => p.id === t.exchange)
  return (
    <Overlay>
      <DialogFrame
        title={title ?? `Proposta por ${name}`}
        width={720}
        onCancel={onCancel}
        onOk={() => onOk(t)}
        cancelLabel="Atrás"
        okLabel={okLabel}
      >
        <div className="pb-2 text-[17px] leading-tight text-white">
          {name}, {club}, está avaliado em {money(value)}.
          <br />
          {budgetText}
        </div>
        <DialogRow wide label="Valor Transferência">
          <MoneyField value={t.fee} onChange={(fee) => set({ fee })} />
          <Presets onPick={(fee) => set({ fee })} />
          <button className="cm-purple h-7 w-8" title="O valor do jogador" onClick={() => set({ fee: value })}>
            =
          </button>
        </DialogRow>
        <DialogRow wide label="Prestações Mensais">
          <Choice value={t.months} options={INSTALMENTS} onChange={(months) => set({ months })} />
        </DialogRow>
        <DialogRow wide label="Percentagem da Próxima Venda">
          <Choice value={t.sellOn} options={SELL_ON} onChange={(sellOn) => set({ sellOn })} />
        </DialogRow>
        <DialogRow wide label="Valor após jogos">
          <MoneyField value={t.appsBonus} onChange={(appsBonus) => set({ appsBonus })} />
          <Presets onPick={(appsBonus) => set({ appsBonus })} />
          <Choice value={t.appsNeeded || 10} options={GAMES} onChange={(appsNeeded) => set({ appsNeeded })} />
        </DialogRow>
        {squad && (
          <>
            <DialogRow wide label="Tornar proposta pública">
              <Choice
                value={t.public}
                options={[
                  [false, 'Não'],
                  [true, 'Sim'],
                ]}
                onChange={(p) => set({ public: p })}
              />
            </DialogRow>
            <DialogRow wide label="Troca">
              <span className="min-w-40 text-[17px] text-cm-yellow">{exchange?.name ?? '-'}</span>
              <button className="cm-purple h-7 w-28" onClick={() => setPicking((v) => !v)}>
                Troca
              </button>
              <button className="cm-purple h-7 w-28" disabled={!exchange} onClick={() => set({ exchange: null })}>
                Remover
              </button>
            </DialogRow>
            {picking && (
              <div className="ml-72 max-h-40 overflow-auto border border-[#7a5a08] bg-[#e7b51f] text-[14px] text-[#1c1400]">
                {squad.map((p) => (
                  <div
                    key={p.id}
                    className="cursor-pointer px-2 hover:bg-[#ffe680]"
                    onClick={() => {
                      set({ exchange: p.id })
                      setPicking(false)
                    }}
                  >
                    {p.name}
                  </div>
                ))}
              </div>
            )}
          </>
        )}
        <DialogRow wide label="Valor Total">
          <span className="text-[17px] font-bold text-cm-yellow">{money(t.fee + t.appsBonus)}</span>
        </DialogRow>
        <div className="pt-2 text-[15px] text-white">O {club} responde dentro de alguns dias.</div>
        {error && <div className="pt-2 text-[15px] font-bold text-[#ffb0a0]">{error}</div>}
      </DialogFrame>
    </Overlay>
  )
}

/** Pedido de empréstimo: duração, parte do salário, taxa mensal e opção de compra. */
export function LoanDialog({
  name,
  club,
  wage,
  initial,
  error,
  onCancel,
  onOk,
}: {
  name: string
  club: string
  wage: string
  initial: LoanTerms
  error?: string | null
  onCancel: () => void
  onOk: (t: LoanTerms) => void
}) {
  const [t, setT] = useState(initial)
  const set = (p: Partial<LoanTerms>) => setT((old) => ({ ...old, ...p }))
  useDialogKeys(onCancel, () => onOk(t))
  return (
    <Overlay>
      <DialogFrame title={`Empréstimo de ${name}`} width={680} onCancel={onCancel} onOk={() => onOk(t)} cancelLabel="Atrás" okLabel="Pedir">
        <div className="pb-2 text-[17px] leading-tight text-white">
          {name}, {club}, ganha {wage}.
        </div>
        <DialogRow wide label="Duração">
          <Choice value={t.months} options={LOAN_LENGTH} onChange={(months) => set({ months })} />
        </DialogRow>
        <DialogRow wide label="Salário Pago">
          <Choice value={t.wageShare} options={WAGE_SHARE} onChange={(wageShare) => set({ wageShare })} />
        </DialogRow>
        <DialogRow wide label="Taxa Mensal">
          <MoneyField value={t.monthlyFee} onChange={(monthlyFee) => set({ monthlyFee })} />
        </DialogRow>
        <DialogRow wide label="Opção de Compra">
          <MoneyField value={t.optionFee} onChange={(optionFee) => set({ optionFee })} />
        </DialogRow>
        <div className="pt-2 text-[15px] text-white">
          Sem opção de compra, deixe o valor a zero. O {club} responde dentro de alguns dias.
        </div>
        {error && <div className="pt-2 text-[15px] font-bold text-[#ffb0a0]">{error}</div>}
      </DialogFrame>
    </Overlay>
  )
}
