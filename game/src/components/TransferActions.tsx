import { useState } from 'react'
import { api } from '../api'
import type { TransferView } from '../bindings/TransferView'
import { useGame } from '../game'
import { FeeDialog, ShopDialog } from './DealDialogs'

// Os diálogos do mercado abertos pelo menu Acção do perfil: o Preço Pedido de um jogador da
// lista e o Oferecer a Clubes (a proposta por um jogador é o `OfferDialog` do perfil).

export type TransferAction = 'price' | 'shop'

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
  // O Oferecer a Clubes a caminho: o botão mostra e não aceita outro clique
  const [sending, setSending] = useState(false)
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
  return (
    <ShopDialog
      name={name}
      value={view.value}
      error={error}
      busy={sending}
      onCancel={onClose}
      onOk={(loan, fee, rivals) =>
        void (async () => {
          setSending(true)
          try {
            const message = await api.offerToClubs(id, loan, fee, rivals)
            onDone(await api.transferView(id), message)
          } catch (e) {
            setError(String(e))
          } finally {
            setSending(false)
          }
        })()
      }
    />
  )
}
