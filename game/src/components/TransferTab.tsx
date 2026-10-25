import { useCallback, useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { api } from '../api'
import type { DealTerms } from '../bindings/DealTerms'
import type { LoanTerms } from '../bindings/LoanTerms'
import type { OfferView } from '../bindings/OfferView'
import type { TransferView } from '../bindings/TransferView'
import { cmDate, money, wage } from '../format'
import { useGame } from '../game'
import { useNav } from '../nav'
import { FeeDialog, LoanDialog, TransferOfferDialog } from './DealDialogs'
import { Link } from './Screen'

// Aba Transferência do perfil do jogador (jogo em curso): valor e contrato, a proposta em
// curso e os botões do CM: Fazer Proposta / Pedir Empréstimo nos outros clubes, Oferecer
// Contrato e as listas de transferências e de empréstimos no plantel do treinador.

const NO_TERMS: DealTerms = { fee: 0, months: 0, sellOn: 0, appsBonus: 0 }
const LOAN_TERMS: LoanTerms = { months: 0, wageShare: 100, monthlyFee: 0, optionFee: 0 }

/** A frase da direção sobre o orçamento, como no CM. */
function budgetText(view: TransferView) {
  return view.budgetNote ?? `A sua direcção autoriza transferências até ${money(view.budget) || '€0'}.`
}

/** Botões de uma proposta (mensagens e perfil); "counter", "offer" e "negotiate" abrem diálogos. */
export function OfferButtons({
  offer,
  onChange,
  onNegotiate,
}: {
  offer: OfferView
  onChange: (o: OfferView) => void
  onNegotiate: () => void
}) {
  const game = useGame()
  const [error, setError] = useState<string | null>(null)
  const [counter, setCounter] = useState(false)
  // "Nova Proposta" / "Novo Pedido": a aba do jogador dá o valor e o orçamento
  const [again, setAgain] = useState<TransferView | null>(null)
  const done = async (o: OfferView) => {
    onChange(o)
    setError(null)
    setCounter(false)
    setAgain(null)
    const info = await api.gameInfo()
    if (info) game.update(info)
  }
  const answer = async (key: string, fee: number | null = null) => {
    try {
      await done(await api.answerOffer(offer.id, key, fee))
    } catch (e) {
      setError(String(e))
    }
  }
  const resend = async (f: () => Promise<TransferView>) => {
    try {
      const v = await f()
      await done(v.offer ?? (await api.offerView(offer.id)))
    } catch (e) {
      setError(String(e))
    }
  }
  const click = (key: string) => {
    if (key === 'counter') setCounter(true)
    else if (key === 'negotiate') onNegotiate()
    else if (key === 'offer') api.transferView(offer.personId).then(setAgain, (e) => setError(String(e)))
    else void answer(key)
  }
  return (
    <div>
      <div className="font-title text-[19px] text-white">{offer.text}</div>
      {offer.actions.length > 0 && (
        <div className="mt-2 flex flex-wrap gap-1">
          {offer.actions.map((a) => (
            <button
              key={a.key}
              className="cm-purple h-7 min-w-36"
              onClick={() => click(a.key)}
            >
              {a.label}
            </button>
          ))}
        </div>
      )}
      {error && <div className="mt-1 text-[15px] font-bold text-[#ff5050]">{error}</div>}
      {counter && (
        <FeeDialog
          title="Pedir Mais"
          name={offer.name}
          value={offer.fee}
          valueLabel="Oferecem"
          feeLabel="Pedir"
          initial={Math.round(offer.fee * 1.25)}
          note={`O ${offer.buyer.shortName} oferece ${money(offer.fee)}. A resposta chega dentro de alguns dias.`}
          error={error}
          onCancel={() => setCounter(false)}
          onOk={(fee) => void answer('counter', fee)}
        />
      )}
      {again && offer.kind === 'loan' && (
        <LoanDialog
          name={offer.name}
          club={offer.seller?.shortName ?? ''}
          wage={wage(again.wage, true)}
          initial={offer.loan}
          error={error}
          onCancel={() => setAgain(null)}
          onOk={(t) => void resend(() => api.askLoan(offer.personId, t))}
        />
      )}
      {again && offer.kind !== 'loan' && (
        <TransferOfferDialog
          name={offer.name}
          club={offer.seller?.shortName ?? ''}
          value={again.value}
          budgetText={budgetText(again)}
          initial={offer.deal}
          error={error}
          onCancel={() => setAgain(null)}
          onOk={(t) => void resend(() => api.makeOffer(offer.personId, t))}
        />
      )}
    </div>
  )
}

export default function TransferTab({ personId, name, onChanged }: { personId: number; name: string; onChanged: () => void }) {
  const game = useGame()
  const nav = useNav()
  const [view, setView] = useState<TransferView | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [dialog, setDialog] = useState<'offer' | 'loan' | 'price' | null>(null)
  const date = game.info?.date

  const load = useCallback(() => api.transferView(personId).then(setView, (e) => setError(String(e))), [personId])
  useEffect(() => {
    if (date) void load()
  }, [load, date])

  const run = async (f: () => Promise<TransferView>) => {
    try {
      setView(await f())
      setError(null)
      setDialog(null)
      const info = await api.gameInfo()
      if (info) game.update(info)
    } catch (e) {
      setError(String(e))
    }
  }

  if (!view) return error ? <div className="p-6 text-center text-cm-magenta">{error}</div> : null
  const rows: [string, ReactNode][] = [
    ['Valor', money(view.value) || '-'],
    ['Salário', wage(view.wage, true)],
    ['Contrato até', view.free ? 'Sem contrato' : cmDate(view.contractExpires)],
  ]
  if (view.optionFee) rows.push(['Opção de compra', money(view.optionFee)])
  if (view.loanFrom) {
    rows.push([
      'Emprestado por',
      <span key="loan">
        <Link onClick={() => nav.go({ kind: 'club', id: view.loanFrom!.id })}>{view.loanFrom.shortName}</Link> até {cmDate(view.loanUntil)}
      </span>,
    ])
  }
  if (view.transferListed || view.loanListed) {
    const lists = [view.transferListed && 'transferências', view.loanListed && 'empréstimos'].filter(Boolean).join(' e ')
    rows.push(['Lista', `De ${lists}${view.asking ? ` (pede ${money(view.asking)})` : ''}`])
  }
  rows.push(['Saldo do clube', money(view.cash) || '€0'])
  rows.push(['Orçamento de transferências', money(view.budget) || '€0'])

  const setList = (transfer: boolean, loan: boolean, asking: number | null) => run(() => api.setListing(personId, transfer, loan, asking))
  const signFree = async () => {
    try {
      await api.makeOffer(personId, NO_TERMS)
      nav.go({ kind: 'contract', id: personId })
    } catch (e) {
      setError(String(e))
    }
  }

  return (
    <div className="mx-auto mt-4 w-full max-w-2xl px-1.5">
      {rows.map(([k, v]) => (
        <div key={k} className="flex h-8 items-center justify-between border-b border-cm-line bg-black/50 px-3 text-[15px]">
          <span className="text-white">{k}</span>
          <span className="font-bold text-cm-yellow">{v}</span>
        </div>
      ))}
      {view.offer && (
        <div className="mt-3 border border-cm-line bg-black/50 px-3 py-2">
          <OfferButtons
            offer={view.offer}
            onChange={() => {
              void load()
              onChanged()
            }}
            onNegotiate={() => nav.go({ kind: 'contract', id: personId })}
          />
        </div>
      )}
      {view.note && <div className="mt-3 text-center text-[15px] text-cm-magenta">{view.note}</div>}
      <div className="mt-3 flex flex-wrap justify-center gap-1">
        {!view.mine && (
          <>
            <button
              className="cm-purple h-8 w-48"
              disabled={!view.canOffer}
              // Um livre: a proposta é o próprio contrato
              onClick={() => (view.free ? void signFree() : setDialog('offer'))}
            >
              {view.free ? 'Oferecer Contrato' : 'Fazer Proposta'}
            </button>
            {!view.free && (
              <button className="cm-purple h-8 w-48" disabled={!view.canLoan} onClick={() => setDialog('loan')}>
                Pedir Empréstimo
              </button>
            )}
            {view.optionFee > 0 && (
              <button className="cm-purple h-8 w-56" disabled={!view.canOption} onClick={() => void run(() => api.exerciseOption(personId))}>
                Exercer Opção de Compra
              </button>
            )}
          </>
        )}
        {view.mine && (
          <>
            <button className="cm-purple h-8 w-48" disabled={!view.canRenew} onClick={() => nav.go({ kind: 'contract', id: personId })}>
              Oferecer Contrato
            </button>
            <button
              className={`cm-purple h-8 w-48 ${view.transferListed ? '!text-cm-yellow' : ''}`}
              onClick={() => void setList(!view.transferListed, view.loanListed, view.asking)}
            >
              Lista de Transferências
            </button>
            <button
              className={`cm-purple h-8 w-48 ${view.loanListed ? '!text-cm-yellow' : ''}`}
              onClick={() => void setList(view.transferListed, !view.loanListed, view.asking)}
            >
              Lista de Empréstimos
            </button>
            <button className="cm-purple h-8 w-48" disabled={!view.transferListed} onClick={() => setDialog('price')}>
              Preço Pedido
            </button>
          </>
        )}
      </div>
      {error && !dialog && <div className="mt-2 text-center text-[15px] font-bold text-[#ff5050]">{error}</div>}
      {dialog === 'price' && (
        <FeeDialog
          title="Preço Pedido"
          feeLabel="Preço"
          name={name}
          value={view.value}
          initial={view.asking ?? view.value}
          note="Os clubes interessados oferecem perto do preço pedido."
          error={error}
          onCancel={() => setDialog(null)}
          onOk={(fee) => void run(() => api.setListing(personId, true, view.loanListed, fee))}
        />
      )}
      {dialog === 'offer' && (
        <TransferOfferDialog
          name={name}
          club={view.club?.shortName ?? ''}
          value={view.value}
          budgetText={budgetText(view)}
          initial={{ ...NO_TERMS, fee: Math.min(view.value, view.budget) || view.value }}
          error={error}
          onCancel={() => setDialog(null)}
          onOk={(t) => void run(() => api.makeOffer(personId, t))}
        />
      )}
      {dialog === 'loan' && (
        <LoanDialog
          name={name}
          club={view.club?.shortName ?? ''}
          wage={wage(view.wage, true)}
          initial={LOAN_TERMS}
          error={error}
          onCancel={() => setDialog(null)}
          onOk={(t) => void run(() => api.askLoan(personId, t))}
        />
      )}
    </div>
  )
}
