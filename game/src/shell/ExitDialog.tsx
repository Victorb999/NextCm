import { useState } from 'react'
import { api } from '../api'
import Loading from '../components/Loading'
import { DialogFrame, Overlay } from '../components/Dialog'
import { useGame } from '../game'

/** Sair do jogo: oferece gravar quando há mudanças por gravar. */
export default function ExitDialog({ onCancel, notify }: { onCancel: () => void; notify: (msg: string) => void }) {
  const game = useGame()
  const unsaved = !!game.info?.unsaved
  // A gravar antes de sair: o ecrã de espera e os botões desligados
  const [saving, setSaving] = useState(false)
  const exit = async (save: boolean) => {
    setSaving(save)
    try {
      if (save) await api.saveGame()
      await game.close()
      onCancel()
    } catch (e) {
      setSaving(false)
      notify(String(e))
    }
  }
  return (
    <Overlay>
      <DialogFrame title="Sair para o Menu" width={520}>
        <div className="py-3 text-center text-[16px] leading-snug text-white">
          {unsaved ? 'Há alterações por gravar. Gravar o jogo antes de sair?' : 'Sair deste jogo?'}
        </div>
        <div className="flex gap-1 pb-1">
          <button className="cm-purple h-8 flex-1 font-title text-[16px]" disabled={saving} onClick={onCancel}>
            Cancelar
          </button>
          {unsaved && (
            <button className="cm-purple h-8 flex-1 font-title text-[16px]" disabled={saving} onClick={() => void exit(false)}>
              Sair sem Gravar
            </button>
          )}
          <button className="cm-purple h-8 flex-1 font-title text-[16px]" disabled={saving} onClick={() => void exit(unsaved)}>
            {unsaved ? 'Gravar e Sair' : 'Sair'}
          </button>
        </div>
      </DialogFrame>
      {saving && <Loading title="Gravar Jogo" detail="A gravar o jogo, por favor aguarde…" delay={150} />}
    </Overlay>
  )
}
