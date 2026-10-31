import { useState } from 'react'
import { api } from '../api'
import LoadFileDialog from './LoadFileDialog'
import TextDialog from './TextDialog'

/**
 * Carregar / Gravar de uma lista de jogadores, como no CM: `slot` 0 é a Lista Preferencial
 * (Carregar junta os jogadores) e 1–7 os olheiros (Carregar troca a lista).
 */
export default function ListFileButtons({
  slot,
  disabled,
  buttonClass = 'cm-purple h-6 w-28',
  onLoaded,
  onMessage,
}: {
  slot: number
  disabled?: boolean
  /** O estilo dos botões (a barra da Pesquisa usa o dela). */
  buttonClass?: string
  onLoaded: () => void
  onMessage: (text: string, error?: boolean) => void
}) {
  const [dialog, setDialog] = useState<'load' | 'save' | null>(null)
  return (
    <>
      <button className={buttonClass} disabled={disabled} onClick={() => setDialog('load')}>
        Carregar
      </button>
      <button className={buttonClass} disabled={disabled} onClick={() => setDialog('save')}>
        Gravar
      </button>
      {dialog === 'load' && (
        <LoadFileDialog
          title="Carregar Lista"
          load={api.listLists}
          empty="Ainda não há listas gravadas. Grave uma com Gravar."
          onCancel={() => setDialog(null)}
          onOk={(file) => {
            setDialog(null)
            api.loadList(slot, file).then(
              () => {
                onMessage(slot === 0 ? 'Lista junta à Lista Preferencial.' : 'Lista carregada.')
                onLoaded()
              },
              (e) => onMessage(String(e), true),
            )
          }}
        />
      )}
      {dialog === 'save' && (
        <TextDialog
          title="Gravar Lista"
          initial=""
          hint="Os jogadores da lista e as estrelas do olheiro."
          onCancel={() => setDialog(null)}
          onOk={(name) => {
            setDialog(null)
            api.saveList(slot, name).then(
              (f) => onMessage(`Lista "${f.name}" gravada.`),
              (e) => onMessage(String(e), true),
            )
          }}
        />
      )}
    </>
  )
}
