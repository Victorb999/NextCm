import { useEffect, useState } from 'react'
import { api } from '../api'
import type { SaveSummary } from '../bindings/SaveSummary'
import { DialogFrame, Overlay } from '../components/Dialog'
import { useGame } from '../game'
import type { ManagerDialog } from './managerMenu'

// Os diálogos do menu do treinador: Gravar Jogo (com a lista de ficheiros e "Ficheiro Existe
// — Substituir?"), Ir de Férias, Rescindir com Clube e Retirar-se.

const HOLIDAYS: [number, string][] = [
  [7, '1 Semana'],
  [14, '2 Semanas'],
  [30, '1 Mês'],
  [90, '3 Meses'],
]

const TEXT = 'py-3 text-center font-title text-[19px] text-white'

export default function ManagerDialogs({
  dialog,
  onClose,
  notify,
}: {
  dialog: ManagerDialog
  onClose: () => void
  notify: (msg: string) => void
}) {
  const game = useGame()
  const [saves, setSaves] = useState<SaveSummary[]>([])
  // O nome do jogo aberto, para gravar por cima
  const [name, setName] = useState(game.info?.name ?? '')
  const [confirm, setConfirm] = useState(false)
  const [busy, setBusy] = useState(false)
  useEffect(() => {
    if (dialog === 'save') api.listSaves().then(setSaves, () => setSaves([]))
  }, [dialog])

  const run = async (f: () => Promise<void>) => {
    setBusy(true)
    try {
      await f()
      onClose()
    } catch (e) {
      const text = String(e)
      if (text.includes('Ficheiro Existe')) setConfirm(true)
      else notify(text)
    } finally {
      setBusy(false)
    }
  }
  const save = (overwrite: boolean) =>
    run(async () => {
      game.update(await api.saveAs(name.trim() || 'Jogo', overwrite))
      notify('Jogo gravado.')
    })

  if (dialog === 'save')
    return (
      <Overlay>
        <DialogFrame title="Gravar Jogo" width={620} onCancel={onClose} onOk={busy ? undefined : () => void save(false)} okLabel="Gravar">
          {confirm ? (
            <div className={TEXT}>
              Ficheiro Existe — Substituir?
              <div className="mt-3 flex gap-1">
                <button className="cm-purple h-8 flex-1" onClick={() => setConfirm(false)}>
                  Não
                </button>
                <button className="cm-purple h-8 flex-1" onClick={() => void save(true)}>
                  Sim
                </button>
              </div>
            </div>
          ) : (
            <>
              <div className="max-h-56 overflow-auto border border-[#1d3a1f] bg-black/60">
                {saves.map((s) => (
                  <div
                    key={s.file}
                    className={`flex cursor-pointer justify-between px-2 text-[15px] hover:bg-cm-sel ${s.name === name ? 'bg-cm-sel' : ''}`}
                    onClick={() => setName(s.name)}
                  >
                    <span className="text-cm-yellow">{s.name}</span>
                    <span className="text-white">
                      {s.managerName} · {s.clubName} · {s.date}
                    </span>
                  </div>
                ))}
                {saves.length === 0 && <div className="p-3 text-center text-cm-magenta">Sem jogos gravados.</div>}
              </div>
              <div className="mt-2 flex items-center gap-2">
                <span className="font-title text-[18px] text-white">Nome</span>
                <input
                  autoFocus
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="Nome do jogo"
                  className="flex-1 border border-[#1d3a1f] bg-black px-2 py-1 text-[17px] text-cm-yellow outline-none"
                />
              </div>
              <div className="pt-1 text-[13px] text-[#bfe6c4]">Escolha um jogo da lista para o substituir, ou escreva um nome novo.</div>
            </>
          )}
        </DialogFrame>
      </Overlay>
    )
  if (dialog === 'holiday')
    return (
      <Overlay>
        <DialogFrame title="Ir de Férias" width={520} onCancel={onClose}>
          <div className={TEXT}>Durante as férias o adjunto escolhe a equipa e os dias passam sem parar.</div>
          <div className="grid grid-cols-2 gap-1 pb-1">
            {HOLIDAYS.map(([days, label]) => (
              <button
                key={days}
                disabled={busy}
                className="cm-purple h-9 font-title text-lg"
                onClick={() => void run(async () => game.update(await api.holiday(days)))}
              >
                {label}
              </button>
            ))}
          </div>
        </DialogFrame>
      </Overlay>
    )
  const resign = dialog === 'resign'
  return (
    <Overlay>
      <DialogFrame
        title={resign ? 'Rescindir com Clube' : 'Retirar-se'}
        width={520}
        cancelLabel="Não"
        okLabel="Sim"
        onCancel={onClose}
        onOk={
          busy
            ? undefined
            : () =>
                void run(async () => {
                  if (resign) {
                    notify(await api.resign())
                    const info = await api.gameInfo()
                    if (info) game.update(info)
                  } else {
                    notify(await api.retireManager())
                    await game.close()
                  }
                })
        }
      >
        <div className={TEXT}>
          {resign
            ? `Rescindir o contrato com o ${game.info?.club.name ?? 'clube'}? Fica sem clube e terá de procurar outro.`
            : 'Retirar-se do futebol? O jogo é gravado e termina aqui.'}
        </div>
      </DialogFrame>
    </Overlay>
  )
}
