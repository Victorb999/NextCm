import { useCallback, useEffect, useRef, useState } from 'react'
import { ChevronDown } from 'lucide-react'
import { createPortal } from 'react-dom'
import type { ReactNode } from 'react'
import type { DealTerms } from '../bindings/DealTerms'
import type { LoanTerms } from '../bindings/LoanTerms'
import { currency, money, moneyNext, wage } from '../format'
import { DialogFrame, DialogRow, Overlay } from './Dialog'
import MoneySelect from './MoneySelect'

// Diálogo verde do mercado: a proposta a um clube (ou o "Pedir Mais", o preço pedido). O
// valor entra na moeda das Definições; o core recebe sempre euros.

/** Esc cancela e Enter confirma, por cima do resto da tela (`off` com outro diálogo por cima). */
function useDialogKeys(onCancel: () => void, onOk: () => void, off = false) {
  useEffect(() => {
    if (off) return
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
const PRESETS = [
  0, 100_000, 500_000, 1_000_000, 5_000_000, 15_000_000, 30_000_000, 50_000_000, 75_000_000, 100_000_000, 125_000_000, 150_000_000,
]

/** Lista amarela do CM, com zebra e linhas de clique fácil, presa por baixo do `anchor` (ou
 *  por cima, sem espaço). Vai para o body por portal: não fica cortada pela rolagem do
 *  diálogo. Fecha ao clicar fora. */
export function YellowList<T>({
  anchor,
  items,
  current,
  label,
  onPick,
  onClose,
}: {
  anchor: HTMLElement
  items: T[]
  current?: T
  label: (v: T) => ReactNode
  onPick: (v: T) => void
  onClose: () => void
}) {
  const box = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const away = (e: MouseEvent) => {
      if (box.current?.contains(e.target as Node) || anchor.contains(e.target as Node)) return
      onClose()
    }
    document.addEventListener('mousedown', away)
    return () => document.removeEventListener('mousedown', away)
  }, [anchor, onClose])
  const r = anchor.getBoundingClientRect()
  const below = window.innerHeight - r.bottom - 8
  const up = below < 160 && r.top > below
  const room = Math.min(300, up ? r.top - 8 : below)
  return createPortal(
    <div
      ref={box}
      style={{
        left: r.left,
        width: r.width,
        maxHeight: room,
        ...(up ? { bottom: window.innerHeight - r.top + 2 } : { top: r.bottom + 2 }),
      }}
      className="fixed z-[80] overflow-auto rounded-sm border border-[#7a5a08] bg-[#e7b51f] py-0.5 text-right text-[15px] text-[#1c1400] shadow-[0_6px_16px_rgba(0,0,0,0.6)]"
    >
      {items.map((v, i) => (
        <div
          key={i}
          className={`flex h-7 cursor-pointer items-center justify-end px-3 even:bg-[#d6a414] hover:!bg-[#ffe680] ${v === current ? 'font-bold' : ''}`}
          onClick={() => {
            onPick(v)
            onClose()
          }}
        >
          {label(v)}
        </div>
      ))}
    </div>,
    document.body,
  )
}

/** Uma linha da proposta como no CM: o rótulo (roxo enquanto a linha não conta, branco
 * quando tem valor) e, à direita, − valor + (e o = do valor do jogador). */
function OfferRow({ label, active, children }: { label: ReactNode; active: boolean; children: ReactNode }) {
  return (
    <div className="flex min-h-[30px] items-center gap-2">
      <span className={`w-80 shrink-0 font-title text-[19px] leading-none ${active ? 'text-white' : 'text-cm-purplelight'}`}>{label}</span>
      <div className="flex items-center gap-1">{children}</div>
    </div>
  )
}

/** − valor ▾ + em euros, com os passos do − / + e a lista amarela do ▾ (na moeda das Definições). */
function MoneyCell({ value, onChange }: { value: number; onChange: (eur: number) => void }) {
  const rate = currency().rate
  const step = (dir: 1 | -1) => onChange(Math.max(0, Math.round(moneyNext(Math.round(value * rate), dir) / rate)))
  // O campo que abriu a lista (null = fechada)
  const [anchor, setAnchor] = useState<HTMLElement | null>(null)
  const close = useCallback(() => setAnchor(null), [])
  return (
    <>
      <button className="cm-purple h-7 w-8" disabled={value <= 0} onClick={() => step(-1)}>
        −
      </button>
      {/* O campo todo abre a lista; a seta fica numa faixa à direita, dentro da borda */}
      <div>
        <button
          className="flex h-7 w-44 items-center overflow-hidden rounded border border-cm-dlgline bg-black text-[16px] tabular-nums text-cm-yellow hover:border-cm-yellow"
          onClick={(e) => {
            const el = e.currentTarget
            setAnchor((a) => (a ? null : el))
          }}
        >
          <span className="flex-1 px-2 text-right">{value > 0 ? money(value) : `${currency().symbol}0`}</span>
          <span className="flex h-full w-6 items-center justify-center border-l border-cm-dlgline bg-cm-purple text-white">
            <ChevronDown size={15} />
          </span>
        </button>
        {anchor && (
          <YellowList
            anchor={anchor}
            items={PRESETS}
            current={value}
            label={(v) => (v > 0 ? money(v) : `${currency().symbol}0`)}
            onPick={onChange}
            onClose={close}
          />
        )}
      </div>
      <button className="cm-purple h-7 w-8" onClick={() => step(1)}>
        +
      </button>
    </>
  )
}

/** 10 → 20 → ... → 50 → 10 jogos. */
const nextGames = (n: number) => (n >= 50 ? 10 : n + 10)
const nextCaps = (n: number) => (n >= 25 ? 5 : n + 5)

const SELL_ON: [number, string][] = Array.from({ length: 11 }, (_, i) => [i * 5, i ? `${i * 5}%` : '-'])
const LOAN_LENGTH: [number, string][] = [
  [0, 'Fim da época'],
  [6, '6 meses'],
  [12, '1 ano'],
]
const WAGE_SHARE: [number, string][] = [0, 25, 50, 75, 100].map((v) => [v, `${v}%`])

/** Um jogador do treinador que pode ir na troca. */
export type ExchangeOption = { id: number; name: string; position: string; age: number | null; wage: number | null }

/** "Adicionar Jogador à Proposta" do CM: os jogadores do plantel que o clube aceita na
 *  troca, ou o aviso de que não tem interesse numa troca. */
function ExchangePicker({
  club,
  squad,
  exchangeable,
  current,
  onCancel,
  onPick,
}: {
  club: string
  squad: ExchangeOption[]
  exchangeable: number[] | null
  current: number | null
  onCancel: () => void
  onPick: (id: number) => void
}) {
  const [chosen, setChosen] = useState<number | null>(current)
  const rows = exchangeable ? squad.filter((p) => exchangeable.includes(p.id)) : []
  useDialogKeys(onCancel, () => chosen !== null && onPick(chosen))
  return (
    <Overlay>
      <DialogFrame
        title="Adicionar Jogador à Proposta"
        width={640}
        onCancel={onCancel}
        onOk={chosen !== null && rows.some((p) => p.id === chosen) ? () => onPick(chosen) : undefined}
        cancelLabel="Atrás"
        okLabel="Escolher"
      >
        {exchangeable === null ? (
          <div className="py-10 text-center text-[17px] text-white">A ver o plantel…</div>
        ) : rows.length === 0 ? (
          <div className="px-6 py-14 text-center font-title text-[20px] text-white">
            O {club} não tem interesse num negócio que envolva troca de jogadores.
          </div>
        ) : (
          <>
            <div className="pb-1.5 text-[15px] text-white">Os jogadores do seu plantel que o {club} aceita na troca:</div>
            <div className="max-h-[330px] overflow-auto rounded bg-black/30">
              <table className="w-full border-collapse text-[15px]">
                <thead className="sticky top-0 bg-cm-head text-[13px] uppercase tracking-wide text-white">
                  <tr>
                    <th className="h-7 px-3 text-left">Nome</th>
                    <th className="w-28 px-2 text-left">Posição</th>
                    <th className="w-16 px-2 text-center">Idade</th>
                    <th className="w-36 px-3 text-right">Salário</th>
                  </tr>
                </thead>
                <tbody>
                  {rows.map((p) => (
                    <tr
                      key={p.id}
                      className={`h-8 cursor-pointer ${p.id === chosen ? 'bg-cm-sel text-white' : 'text-cm-text hover:bg-cm-dlghover'}`}
                      onClick={() => setChosen(p.id)}
                      onDoubleClick={() => onPick(p.id)}
                    >
                      <td className="px-3 font-semibold">{p.name}</td>
                      <td className="px-2">{p.position}</td>
                      <td className="px-2 text-center tabular-nums">{p.age ?? '-'}</td>
                      <td className="px-3 text-right tabular-nums">{wage(p.wage)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </>
        )}
      </DialogFrame>
    </Overlay>
  )
}

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
  exchangeable = null,
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
  /** Jogadores do plantel para a troca; sem lista, não há troca nem proposta pública ("Pedir Mais"). */
  squad?: ExchangeOption[]
  /** Quais deles o clube aceita (null enquanto carrega). */
  exchangeable?: number[] | null
  okLabel?: string
  error?: string | null
  onCancel: () => void
  onOk: (t: DealTerms) => void
}) {
  const [t, setT] = useState(initial)
  const set = (p: Partial<DealTerms>) => setT((old) => ({ ...old, ...p }))
  const [picking, setPicking] = useState(false)
  useDialogKeys(onCancel, () => onOk(t), picking)
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
          <OfferRow
            label={
              <>
                Valor após{' '}
                <button
                  className="underline decoration-dotted underline-offset-4 hover:text-cm-yellow"
                  title="Mudar o número de internacionalizações"
                  onClick={() => set({ capsNeeded: nextCaps(t.capsNeeded || 5) })}
                >
                  {t.capsNeeded || 5}
                </button>{' '}
                internacionalizações
              </>
            }
            active={t.capsBonus > 0}
          >
            <MoneyCell value={t.capsBonus} onChange={(capsBonus) => set({ capsBonus })} />
          </OfferRow>
          {squad && (
            <OfferRow label="Tornar proposta pública" active={t.public}>
              <span className="w-8" />
              <button
                className="h-7 w-44 text-center text-[17px] text-cm-yellow hover:text-white"
                onClick={() => set({ public: !t.public })}
              >
                {t.public ? 'Sim' : 'Não'}
              </button>
            </OfferRow>
          )}
        </div>
        {squad && (
          <div className="relative mt-3 flex items-center gap-1 rounded bg-black/30 p-1">
            <button className="cm-purple h-7 w-32" onClick={() => setPicking(true)}>
              Troca
            </button>
            <button className="cm-purple h-7 w-32" disabled={!exchange} onClick={() => set({ exchange: null })}>
              Remover
            </button>
            <span className="ml-3 truncate text-[17px] text-cm-yellow">
              {exchange ? (
                `${exchange.name} (${exchange.position}) vai na troca`
              ) : (
                <span className="text-[15px] text-cm-purplelight">Nenhum jogador na troca</span>
              )}
            </span>
          </div>
        )}
        <div className="mt-3 flex items-center">
          <span className="w-80 font-title text-[19px] text-white">Valor Total</span>
          <span className="text-[19px] font-bold text-cm-yellow">{money(t.fee + t.appsBonus + t.capsBonus)}</span>
        </div>
        <div className="pt-2 text-[15px] text-white">O {club} responde dentro de alguns dias.</div>
        {error && <div className="pt-2 text-[15px] font-bold text-[#ffb0a0]">{error}</div>}
      </DialogFrame>
      {picking && squad && (
        <ExchangePicker
          club={club}
          squad={squad}
          exchangeable={exchangeable}
          current={t.exchange}
          onCancel={() => setPicking(false)}
          onPick={(id) => {
            set({ exchange: id })
            setPicking(false)
          }}
        />
      )}
    </Overlay>
  )
}

const SHOP_KIND: [boolean, string][] = [
  [false, 'Transferência'],
  [true, 'Empréstimo'],
]

/** "Oferecer a Clubes" (do FM): transferência ou empréstimo, o valor pedido e se os rivais
 *  ficam de fora. Os serviços do clube mandam a proposta a todos os clubes. */
export function ShopDialog({
  name,
  value,
  error,
  onCancel,
  onOk,
}: {
  name: string
  value: number
  error?: string | null
  onCancel: () => void
  onOk: (loan: boolean, fee: number, excludeRivals: boolean) => void
}) {
  const [loan, setLoan] = useState(false)
  const [fee, setFee] = useState(value)
  const [rivals, setRivals] = useState(true)
  useDialogKeys(onCancel, () => onOk(loan, fee, rivals))
  return (
    <Overlay>
      <DialogFrame
        title="Oferecer a Clubes"
        width={680}
        onCancel={onCancel}
        onOk={() => onOk(loan, fee, rivals)}
        cancelLabel="Atrás"
        okLabel="Fazer Proposta"
      >
        <div className="pb-2 text-[17px] leading-tight text-white">
          {name} está avaliado em {money(value)}.
        </div>
        <div className="flex flex-col gap-1">
          <OfferRow label="Tipo de Proposta" active>
            <Choice value={loan} options={SHOP_KIND} onChange={setLoan} />
          </OfferRow>
          {!loan && (
            <OfferRow label="Valor Pedido" active={fee > 0}>
              <MoneyCell value={fee} onChange={setFee} />
              <button className="cm-purple h-7 w-8" title="O valor do jogador" onClick={() => setFee(value)}>
                =
              </button>
            </OfferRow>
          )}
          <OfferRow label="Excluir clubes rivais" active={rivals}>
            <span className="w-8" />
            <button className="h-7 w-44 text-center text-[17px] text-cm-yellow hover:text-white" onClick={() => setRivals((r) => !r)}>
              {rivals ? 'Sim' : 'Não'}
            </button>
          </OfferRow>
        </div>
        <div className="mt-3 rounded bg-black/30 px-3 py-2 text-[15px] leading-snug text-cm-green">
          {loan ? (
            <div>- Um empréstimo: o clube que o quiser diz que parte do salário paga.</div>
          ) : (
            <div>- Valor potencial da transferência: {money(fee > 0 ? fee : value)}.</div>
          )}
          <div>
            - Os serviços administrativos do clube vão enviar esta proposta a todos os clubes
            {rivals ? ' (menos os rivais)' : ''}. Será contactado nos próximos dias se houver interesse.
          </div>
        </div>
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
