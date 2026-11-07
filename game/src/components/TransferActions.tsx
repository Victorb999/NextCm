import { useEffect, useState } from 'react'
import { api } from '../api'
import type { TransferView } from '../bindings/TransferView'
import { wage } from '../format'
import { useGame } from '../game'
import { LOAN_TERMS, NO_TERMS, budgetText } from '../transferTerms'
import { FeeDialog, LoanDialog, ShopDialog, TransferOfferDialog } from './DealDialogs'
import type { ExchangeOption } from './DealDialogs'

// Os diálogos do mercado abertos pelo menu Acção do perfil, como no CM: Abordar Para Comprar,
// Propôr Empréstimo e o Preço Pedido de um jogador da lista.

export type TransferAction = 'offer' | 'loan' | 'price' | 'shop'

export default function TransferActions({
  action,
  view,
  name,
  onClose,
  onDone,
}: {
  action: TransferAction
  view: TransferView
  name: string
  onClose: () => void
  /** `message`: o texto de uma acção sem resposta imediata (o Oferecer a Clubes). */
  onDone: (v: TransferView, message?: string) => void
}) {
  const game = useGame()
  const [error, setError] = useState<string | null>(null)
  // A troca: o plantel do treinador e quem dele o clube vendedor aceita (decide o core)
  const [squad, setSquad] = useState<ExchangeOption[]>([])
  const [exchangeable, setExchangeable] = useState<number[] | null>(null)
  const club = game.info?.club.id
  const person = view.personId
  useEffect(() => {
    if (action !== 'offer' || !club) return
    api.getClub(club).then(
      (c) => setSquad(c.players.map((p) => ({ id: p.id, name: p.listName, position: p.position, age: p.age, wage: p.wage }))),
      () => {},
    )
    api.exchangeCandidates(person).then(setExchangeable, () => setExchangeable([]))
  }, [action, club, person])

  const run = async (f: () => Promise<TransferView>) => {
    try {
      onDone(await f())
      const info = await api.gameInfo()
      if (info) game.update(info)
    } catch (e) {
      setError(String(e))
    }
  }
  const id = view.personId
  // Emprestado ao clube do treinador: o vendedor é o clube dono
  const seller = (view.club?.id === club && view.loanFrom ? view.loanFrom : view.club)?.shortName ?? ''
  if (action === 'price')
    return (
      <FeeDialog
        title="Preço Pedido"
        feeLabel="Preço"
        name={name}
        value={view.value}
        initial={view.asking ?? view.value}
        note="Os clubes interessados oferecem perto do preço pedido."
        error={error}
        onCancel={onClose}
        onOk={(fee) => void run(() => api.setListing(id, true, view.loanListed, fee))}
      />
    )
  if (action === 'shop')
    return (
      <ShopDialog
        name={name}
        value={view.value}
        error={error}
        onCancel={onClose}
        onOk={(loan, fee, rivals) =>
          void (async () => {
            try {
              const message = await api.offerToClubs(id, loan, fee, rivals)
              onDone(await api.transferView(id), message)
            } catch (e) {
              setError(String(e))
            }
          })()
        }
      />
    )
  if (action === 'loan')
    return (
      <LoanDialog
        name={name}
        club={seller}
        wage={wage(view.wage, true)}
        initial={LOAN_TERMS}
        error={error}
        onCancel={onClose}
        onOk={(t) => void run(() => api.askLoan(id, t))}
      />
    )
  return (
    <TransferOfferDialog
      name={name}
      club={seller}
      value={view.value}
      budgetText={budgetText(view)}
      initial={{ ...NO_TERMS, fee: Math.min(view.value, view.budget) || view.value }}
      squad={squad}
      exchangeable={exchangeable}
      error={error}
      onCancel={onClose}
      onOk={(t) => void run(() => api.makeOffer(id, t))}
    />
  )
}
