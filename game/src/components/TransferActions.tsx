import { useEffect, useState } from 'react'
import { api } from '../api'
import type { TransferView } from '../bindings/TransferView'
import { wage } from '../format'
import { useGame } from '../game'
import { LOAN_TERMS, NO_TERMS, budgetText } from '../transferTerms'
import { FeeDialog, LoanDialog, TransferOfferDialog } from './DealDialogs'

// Os diálogos do mercado abertos pelo menu Acção do perfil, como no CM: Abordar Para Comprar,
// Propôr Empréstimo e o Preço Pedido de um jogador da lista.

export type TransferAction = 'offer' | 'loan' | 'price'

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
  onDone: (v: TransferView) => void
}) {
  const game = useGame()
  const [error, setError] = useState<string | null>(null)
  // Quem pode ir na troca: o plantel do treinador
  const [squad, setSquad] = useState<{ id: number; name: string }[]>([])
  const club = game.info?.club.id
  useEffect(() => {
    if (action === 'offer' && club)
      api.getClub(club).then((c) => setSquad(c.players.map((p) => ({ id: p.id, name: p.listName }))), () => {})
  }, [action, club])

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
  const seller = view.club?.shortName ?? ''
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
      error={error}
      onCancel={onClose}
      onOk={(t) => void run(() => api.makeOffer(id, t))}
    />
  )
}
