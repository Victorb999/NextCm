import { useCallback, useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { api } from '../api'
import type { OfferView } from '../bindings/OfferView'
import type { TransferView } from '../bindings/TransferView'
import { cmDate, money, wage } from '../format'
import { useGame } from '../game'
import { useNav } from '../nav'
import { budgetText } from '../transferTerms'
import { LoanDialog, TransferOfferDialog } from './DealDialogs'
import { Link } from './Screen'

// Aba Transferência do perfil do jogador (jogo em curso), como no CM: disponibilidade, valor,
// contrato, propostas, interessados e o futuro, com a proposta em curso e os botões dela. As
// acções (Abordar Para Comprar, Propôr Empréstimo, as listas...) estão no menu Acção.

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
  const ask = async (t: OfferView['deal']) => {
    try {
      await done(await api.askTerms(offer.id, t))
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
        <TransferOfferDialog
          title="Pedir Mais"
          okLabel="Pedir"
          name={offer.name}
          club={offer.buyer.shortName}
          value={offer.fee}
          budgetText={`O ${offer.buyer.shortName} oferece ${money(offer.fee)}. Peça mais, uma percentagem da próxima venda, menos prestações ou um valor por jogos.`}
          initial={{ ...offer.deal, fee: Math.round(offer.fee * 1.25) }}
          error={error}
          onCancel={() => setCounter(false)}
          onOk={(t) => void ask(t)}
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

/** `version` muda depois de uma acção do menu Acção: a aba volta a carregar. */
export default function TransferTab({ personId, version, onChanged }: { personId: number; version: number; onChanged: () => void }) {
  const game = useGame()
  const nav = useNav()
  const [view, setView] = useState<TransferView | null>(null)
  const [error, setError] = useState<string | null>(null)
  const date = game.info?.date

  const load = useCallback(() => api.transferView(personId).then(setView, (e) => setError(String(e))), [personId])
  useEffect(() => {
    if (date) void load()
  }, [load, date, version])

  if (!view) return error ? <div className="p-6 text-center text-cm-magenta">{error}</div> : null
  const rows: [string, ReactNode][] = [
    ['Disponibilidade', view.availability],
    ['Valor', money(view.value) || '-'],
    ['Salário', wage(view.wage, true)],
    ['Contrato até', view.free ? 'Sem contrato' : cmDate(view.contractExpires)],
    ['Propostas', view.offers.length ? view.offers.join(', ') : 'Nenhuma'],
    [
      'Interessados',
      view.interested.length
        ? view.interested.map((c, i) => (
            <span key={c.id}>
              {i > 0 && ', '}
              <Link onClick={() => nav.go({ kind: 'club', id: c.id })}>{c.shortName}</Link>
            </span>
          ))
        : 'Nenhum',
    ],
  ]
  if (view.future) rows.push(['Futuro', view.future])
  if (view.optionFee) rows.push(['Opção de compra', money(view.optionFee)])
  if (view.loanFrom) {
    rows.push([
      'Emprestado por',
      <span key="loan">
        <Link onClick={() => nav.go({ kind: 'club', id: view.loanFrom!.id })}>{view.loanFrom.shortName}</Link> até {cmDate(view.loanUntil)}
      </span>,
    ])
  }
  rows.push(['Saldo do clube', money(view.cash) || '€0'])
  rows.push(['Orçamento de transferências', money(view.budget) || '€0'])

  return (
    <div className="mx-auto mt-4 w-full max-w-3xl px-1.5">
      {rows.map(([k, v]) => (
        <div key={k} className="flex min-h-8 items-center justify-between gap-4 border-b border-cm-line bg-black/50 px-3 text-[15px]">
          <span className="shrink-0 text-white">{k}</span>
          <span className="text-right font-bold text-cm-yellow">{v}</span>
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
      {error && <div className="mt-2 text-center text-[15px] font-bold text-[#ff5050]">{error}</div>}
    </div>
  )
}
