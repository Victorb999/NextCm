import { useState } from 'react'
import { api } from '../api'
import Loading from '../components/Loading'
import { DialogFrame, Overlay } from '../components/Dialog'
import { useGame } from '../game'

/** Sair do jogo: oferece gravar quando há mudanças por gravar. `restart`: o Reiniciar Jogo, que
 * depois de sair abre o Novo Jogo. */
export default function ExitDialog({
  onCancel,
  notify,
  restart = false,
}: {
  onCancel: () => void
  notify: (msg: string) => void
  restart?: boolean
}) {
  const game = useGame()
  const unsaved = !!game.info?.unsaved
  // A gravar antes de sair: o ecrã de espera e os botões desligados
  const [saving, setSaving] = useState(false)
  const exit = async (save: boolean) => {
    setSaving(save)
    try {
      if (save) await api.saveGame()
      await game.close()
      if (restart) game.setMode('newGame')
      onCancel()
    } catch (e) {
      setSaving(false)
      notify(String(e))
    }
  }
  return (
    <Overlay>
      <DialogFrame title={restart ? 'Reiniciar Jogo' : 'Sair para o Menu'} width={520}>
        <div className="py-3 text-center text-[16px] leading-snug text-white">
          {restart
            ? unsaved
              ? 'Começar um jogo novo? Há alterações por salvar neste.'
              : 'Deixar este jogo e começar um novo?'
            : unsaved
              ? 'Há alterações por salvar. Salvar o jogo antes de sair?'
              : 'Sair deste jogo?'}
        </div>
        <div className="flex gap-1 pb-1">
          <button className="cm-purple h-8 flex-1 font-title text-[16px]" disabled={saving} onClick={onCancel}>
            Cancelar
          </button>
          {unsaved && (
            <button className="cm-purple h-8 flex-1 font-title text-[16px]" disabled={saving} onClick={() => void exit(false)}>
              {restart ? 'Reiniciar sem Salvar' : 'Sair sem Salvar'}
            </button>
          )}
          <button className="cm-purple h-8 flex-1 font-title text-[16px]" disabled={saving} onClick={() => void exit(unsaved)}>
            {restart ? (unsaved ? 'Salvar e Reiniciar' : 'Reiniciar') : unsaved ? 'Salvar e Sair' : 'Sair'}
          </button>
        </div>
      </DialogFrame>
      {saving && <Loading title="Salvar Jogo" detail="Salvando o jogo, por favor aguarde…" delay={150} />}
    </Overlay>
  )
}
