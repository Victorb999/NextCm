import { useCallback, useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { api } from '../api'
import type { OfferView } from '../bindings/OfferView'
import type { TransferView } from '../bindings/TransferView'
import { cmDate, money, wage } from '../format'
import { useGame } from '../game'
import { useNav } from '../nav'
import { FeeDialog } from './DealDialogs'
import { Link } from './Screen'

// Aba Transferência do perfil do jogador (jogo em curso): valor e contrato, a proposta em
// curso e os botões do CM: Fazer Proposta / Pedir Empréstimo nos outros clubes, Oferecer
// Contrato e as listas de transferências e de empréstimos no plantel do treinador.

/** Botões de uma proposta (mensagens e perfil); "counter" e "negotiate" abrem diálogos. */
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
  const answer = async (key: string, fee: number | null = null) => {
    try {
      onChange(await api.answerOffer(offer.id, key, fee))
      setError(null)
      setCounter(false)
      const info = await api.gameInfo()
      if (info) game.update(info)
    } catch (e) {
      setError(String(e))
    }
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
              onClick={() => (a.key === 'counter' ? setCounter(true) : a.key === 'negotiate' ? onNegotiate() : void answer(a.key))}
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
          note={`O ${offer.buyer.shortName} oferece ${money(offer.fee)}. A resposta chega amanhã.`}
          error={error}
          onCancel={() => setCounter(false)}
          onOk={(fee) => void answer('counter', fee)}
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
  const [dialog, setDialog] = useState<'offer' | null>(null)
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
      await api.makeOffer(personId, 0)
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
              <button className="cm-purple h-8 w-48" disabled={!view.canLoan} onClick={() => void run(() => api.askLoan(personId))}>
                Pedir Empréstimo
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
            <button className="cm-purple h-8 w-48" disabled={!view.transferListed} onClick={() => setDialog('offer')}>
              Preço Pedido
            </button>
          </>
        )}
      </div>
      {error && !dialog && <div className="mt-2 text-center text-[15px] font-bold text-[#ff5050]">{error}</div>}
      {dialog === 'offer' && (
        <FeeDialog
          title={view.mine ? 'Preço Pedido' : 'Fazer Proposta'}
          feeLabel={view.mine ? 'Preço' : 'Proposta'}
          name={name}
          value={view.value}
          initial={view.mine ? (view.asking ?? view.value) : view.value}
          note={
            view.mine
              ? 'Os clubes interessados oferecem perto do preço pedido.'
              : (view.budgetNote ?? `A sua direcção autoriza transferências até ${money(view.budget) || '€0'}.`) + ' O clube responde amanhã.'
          }
          error={error}
          onCancel={() => setDialog(null)}
          onOk={(fee) => void run(() => (view.mine ? api.setListing(personId, true, view.loanListed, fee) : api.makeOffer(personId, fee)))}
        />
      )}
    </div>
  )
}
