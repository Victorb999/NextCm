import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import type { DealTerms } from '../bindings/DealTerms'
import type { LoanTerms } from '../bindings/LoanTerms'
import { currency, money, moneyNext } from '../format'
import { DialogFrame, DialogRow, Overlay } from './Dialog'
import MoneySelect from './MoneySelect'

// Diálogo verde do mercado: a proposta a um clube (ou o "Pedir Mais", o preço pedido). O
// valor entra na moeda das Definições; o core recebe sempre euros.

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
          <MoneySelect value={fee} onChange={setFee} />
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
      <span className="w-44 text-center text-[17px] text-cm-yellow">{options[i][1]}</span>
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

/** Uma linha da proposta como no CM: o rótulo (roxo enquanto a linha não conta, branco
 * quando tem valor) e, à direita, − valor + (e o = do valor do jogador). */
function OfferRow({ label, active, children }: { label: ReactNode; active: boolean; children: ReactNode }) {
  return (
    <div className="flex min-h-[30px] items-center gap-2">
      <span className={`w-80 shrink-0 font-title text-[19px] leading-none ${active ? 'text-white' : 'text-[#b48cf0]'}`}>{label}</span>
      <div className="flex items-center gap-1">{children}</div>
    </div>
  )
}

/** − valor ▾ + em euros, com os passos do − / + e a lista amarela do ▾ (na moeda das Definições). */
function MoneyCell({ value, onChange }: { value: number; onChange: (eur: number) => void }) {
  const rate = currency().rate
  const step = (dir: 1 | -1) => onChange(Math.max(0, Math.round(moneyNext(Math.round(value * rate), dir) / rate)))
  return (
    <>
      <button className="cm-purple h-7 w-8" disabled={value <= 0} onClick={() => step(-1)}>
        −
      </button>
      <div className="flex h-7 w-44 items-center justify-end gap-1 rounded border border-[#2f6b52] bg-black px-1 text-[16px] tabular-nums text-cm-yellow">
        <span className="flex-1 text-right">{value > 0 ? money(value) : `${currency().symbol}0`}</span>
        <Presets onPick={onChange} />
      </div>
      <button className="cm-purple h-7 w-8" onClick={() => step(1)}>
        +
      </button>
    </>
  )
}

/** 10 → 20 → ... → 50 → 10 jogos. */
const nextGames = (n: number) => (n >= 50 ? 10 : n + 10)

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
        <div className="flex flex-col gap-1">
          <OfferRow label="Valor Transferência" active={t.fee > 0}>
            <MoneyCell value={t.fee} onChange={(fee) => set({ fee })} />
            <button className="cm-purple h-7 w-8" title="O valor do jogador" onClick={() => set({ fee: value })}>
              =
            </button>
          </OfferRow>
          <OfferRow label="Prestações Mensais" active={t.months > 0}>
            <Choice value={t.months} options={INSTALMENTS} onChange={(months) => set({ months })} />
          </OfferRow>
          <OfferRow label="Percentagem da Próxima Venda" active={t.sellOn > 0}>
            <Choice value={t.sellOn} options={SELL_ON} onChange={(sellOn) => set({ sellOn })} />
          </OfferRow>
          <OfferRow
            label={
              <>
                Valor após{' '}
                <button
                  className="underline decoration-dotted underline-offset-4 hover:text-cm-yellow"
                  title="Mudar o número de jogos"
                  onClick={() => set({ appsNeeded: nextGames(t.appsNeeded || 10) })}
                >
                  {t.appsNeeded || 10}
                </button>{' '}
                jogos no campeonato
              </>
            }
            active={t.appsBonus > 0}
          >
            <MoneyCell value={t.appsBonus} onChange={(appsBonus) => set({ appsBonus })} />
          </OfferRow>
          {squad && (
            <OfferRow label="Tornar proposta pública" active={t.public}>
              <span className="w-8" />
              <button className="h-7 w-44 text-center text-[17px] text-cm-yellow hover:text-white" onClick={() => set({ public: !t.public })}>
                {t.public ? 'Sim' : 'Não'}
              </button>
            </OfferRow>
          )}
        </div>
        {squad && (
          <div className="relative mt-3 flex items-center gap-1 rounded bg-black/30 p-1">
            <button className="cm-purple h-7 w-32" onClick={() => setPicking((v) => !v)}>
              Troca
            </button>
            <button className="cm-purple h-7 w-32" disabled={!exchange} onClick={() => set({ exchange: null })}>
              Remover
            </button>
            <span className="ml-3 truncate text-[17px] text-cm-yellow">{exchange ? `${exchange.name} vai na troca` : ''}</span>
            {picking && (
              <div className="absolute left-1 top-9 z-10 max-h-48 w-64 overflow-auto border border-[#7a5a08] bg-[#e7b51f] text-[14px] text-[#1c1400]">
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
          </div>
        )}
        <div className="mt-3 flex items-center">
          <span className="w-80 font-title text-[19px] text-white">Valor Total</span>
          <span className="text-[19px] font-bold text-cm-yellow">{money(t.fee + t.appsBonus)}</span>
        </div>
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
          <MoneySelect value={t.monthlyFee} onChange={(monthlyFee) => set({ monthlyFee })} />
        </DialogRow>
        <DialogRow wide label="Opção de Compra">
          <MoneySelect value={t.optionFee} onChange={(optionFee) => set({ optionFee })} />
        </DialogRow>
        <div className="pt-2 text-[15px] text-white">
          Sem opção de compra, deixe o valor a zero. O {club} responde dentro de alguns dias.
        </div>
        {error && <div className="pt-2 text-[15px] font-bold text-[#ffb0a0]">{error}</div>}
      </DialogFrame>
    </Overlay>
  )
}
