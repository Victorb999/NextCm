import { useEffect, useState } from 'react'
import { api } from '../api'
import type { SaveSummary } from '../bindings/SaveSummary'
import { DialogFrame, Overlay } from '../components/Dialog'
import { cmDate } from '../format'
import { useGame } from '../game'
import type { ManagerDialog } from './managerMenu'
import { Empty } from '../components/Screen'

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
              <div className="pb-2 text-center font-title text-[18px] font-bold text-cm-yellow">Escolha Nome Ficheiro</div>
              <div className="flex items-center gap-2 pb-2">
                <span className="w-16 text-[15px] font-semibold text-white">Nome</span>
                <input
                  autoFocus
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="Nome do jogo"
                  className="h-8 flex-1 rounded border border-[#2f6b52] bg-black px-2 text-[16px] text-cm-yellow outline-none focus:border-cm-yellow"
                />
              </div>
              <div className="max-h-56 overflow-auto rounded bg-black/40">
                <div className="sticky top-0 flex h-7 items-center gap-2 bg-[#3d1a73] px-3 font-mono text-[11px] font-bold uppercase tracking-[0.1em] text-cm-yellow">
                  <span className="flex-1">Jogo</span>
                  <span className="w-48">Clube</span>
                  <span className="w-24 text-right">Data do jogo</span>
                </div>
                <div className="cm-zebra">
                  {saves.map((s) => (
                    <div
                      key={s.file}
                      className={`flex h-7 cursor-pointer items-center gap-2 px-3 text-[14px] hover:text-cm-yellow ${s.name === name ? '!bg-cm-sel' : ''}`}
                      onClick={() => setName(s.name)}
                    >
                      <span className="min-w-0 flex-1 truncate font-semibold text-white">{s.name}</span>
                      <span className="w-48 truncate text-cm-magenta">{s.clubName}</span>
                      <span className="w-24 text-right text-cm-yellow">{cmDate(s.date)}</span>
                    </div>
                  ))}
                </div>
                {saves.length === 0 && <Empty className="p-3">Sem jogos gravados.</Empty>}
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
