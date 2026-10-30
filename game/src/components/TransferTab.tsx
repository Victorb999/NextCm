import { useCallback, useEffect, useState } from 'react'
import { createPortal } from 'react-dom'
import type { ReactNode } from 'react'
import { api } from '../api'
import type { OfferView } from '../bindings/OfferView'
import type { TransferView } from '../bindings/TransferView'
import { cmDate, money, wage } from '../format'
import { useGame } from '../game'
import { useNav } from '../nav'
import { budgetText } from '../transferTerms'
import { LoanDialog, TransferOfferDialog } from './DealDialogs'
import { GREEN_FIELD, Overlay } from './Dialog'
import { Empty, Link } from './Screen'

// Painel Mercado da aba Contrato & Transferência do perfil (jogo em curso), como no CM:
// disponibilidade, valor, propostas, interessados e o futuro, com a proposta em curso e os
// botões dela. As acções (Abordar Para Comprar, as listas...) vêm do perfil.

/** Botões de uma proposta (mensagens e perfil); "counter", "offer" e "negotiate" abrem diálogos. */
export function OfferButtons({
  offer,
  onChange,
  onNegotiate,
  actionsTarget,
}: {
  offer: OfferView
  onChange: (o: OfferView) => void
  onNegotiate: () => void
  /** Os botões noutro sítio (a barra da lista das Notícias, como no CM). */
  actionsTarget?: HTMLElement | null
}) {
  const game = useGame()
  const [error, setError] = useState<string | null>(null)
  const [counter, setCounter] = useState(false)
  // A venda ou a compra à espera do treinador: "Confirmar" abre a pergunta do CM (Não / Adiar / Sim)
  const [confirmSale, setConfirmSale] = useState(false)
  const selling = offer.incoming && offer.status === 'confirm'
  const buying = offer.kind === 'contract' && offer.status === 'agreed' && offer.seller?.id !== offer.buyer.id
  const asking = selling || buying
  const question = selling
    ? `Vender ${offer.name} ao ${offer.buyer.shortName} por ${money(offer.fee)}?`
    : offer.fee > 0
      ? `Contratar ${offer.name} por ${money(offer.fee)}?`
      : `Contratar ${offer.name} a custo zero?`
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
  const portal = (node: ReactNode) => (actionsTarget ? createPortal(node, actionsTarget) : node)
  const click = (key: string) => {
    if (asking && key === 'confirm') setConfirmSale(true)
    else if (key === 'counter') setCounter(true)
    else if (key === 'negotiate') onNegotiate()
    else if (key === 'offer') api.transferView(offer.personId).then(setAgain, (e) => setError(String(e)))
    else void answer(key)
  }
  return (
    <div>
      <div className="font-title text-[19px] text-white">{offer.text}</div>
      {offer.actions.length > 0 && portal(
        <div className={actionsTarget ? 'flex gap-1' : 'mt-2 flex flex-wrap gap-1'}>
          {offer.actions.filter((a) => !asking || a.key === 'confirm').map((a) => (
            <button
              key={a.key}
              className={actionsTarget ? 'cm-purple h-6 min-w-32 px-3' : 'cm-purple h-7 min-w-36'}
              onClick={() => click(a.key)}
            >
              {a.label}
            </button>
          ))}
        </div>,
      )}
      {error && <div className="mt-1 text-[15px] font-bold text-[#ff5050]">{error}</div>}
      {confirmSale && (
        <Overlay>
          <div className="flex w-[640px] max-w-full flex-col border-2 border-[#1d4a20] bg-[#0b3a10] p-2 shadow-[4px_4px_0_rgba(0,0,0,0.6)]">
            <div className="flex h-12 items-center justify-center border-2 border-[#333] bg-black font-title text-2xl text-white">
              Confirmar Proposta Transferência
            </div>
            <div className="mt-1.5 border-2 border-[#1d3a1f] px-3 py-8 text-center font-title text-[19px] text-white" style={{ background: GREEN_FIELD }}>
              {question}
            </div>
            <div className="mt-2 flex gap-1">
              <button className="cm-purple h-9 flex-1 font-title text-xl" onClick={() => (setConfirmSale(false), void answer(selling ? 'reject' : 'withdraw'))}>
                Não
              </button>
              <button className="cm-purple h-9 flex-1 font-title text-xl" title="Decidir depois (até ao prazo)" onClick={() => setConfirmSale(false)}>
                Adiar
              </button>
              <button className="cm-purple h-9 flex-1 font-title text-xl" onClick={() => (setConfirmSale(false), void answer('confirm'))}>
                Sim
              </button>
            </div>
          </div>
        </Overlay>
      )}
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

  if (!view) return error ? <Empty>{error}</Empty> : null
  const rows: [string, ReactNode][] = [
    ['Disponibilidade', view.availability],
    ['Valor', money(view.value) || '-'],
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
    <div>
      <div className="cm-zebra">
        {rows.map(([k, v]) => (
          <div key={k} className="flex min-h-6 items-center justify-between gap-4 px-3 text-[13.5px]">
            <span className="shrink-0 text-[#c8c8d0]">{k}</span>
            <span className="text-right font-bold text-cm-yellow">{v}</span>
          </div>
        ))}
      </div>
      {view.offer && (
        <div className="border-t border-cm-line px-3 py-2">
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
      {view.note && <div className="border-t border-cm-line px-3 py-1.5 text-center text-[14px] text-cm-magenta">{view.note}</div>}
      {error && <div className="px-3 py-1.5 text-center text-[14px] font-bold text-[#ff5050]">{error}</div>}
    </div>
  )
}
