import { useState } from 'react'
import { api } from '../../api'
import type { EditorCommand } from '../../bindings/EditorCommand'
import { DialogFrame, Overlay } from '../../components/Dialog'
import { useLoad } from '../../useLoad'

/** O "Custom Command" do editor do CM: os comandos marcados aplicam-se à lista da esquerda (ou só
 * ao jogador aberto), de uma vez. Quais existem e o que fazem decide o core. */
export default function CommandDialog({
  count,
  person,
  onRun,
  onClose,
}: {
  count: number
  person: string | null
  onRun: (commands: string[], firstTeam: boolean, onlyPerson: boolean) => void
  onClose: () => void
}) {
  const [list, setList] = useState<EditorCommand[]>([])
  const [failed, setFailed] = useState<string | null>(null)
  const [picked, setPicked] = useState<string[]>([])
  const [firstTeam, setFirstTeam] = useState(false)
  const [onlyPerson, setOnlyPerson] = useState(false)
  useLoad(() => api.editorCommands(), [], setList, setFailed)
  const toggle = (key: string, on: boolean) => setPicked((p) => (on ? [...p.filter((k) => k !== key), key] : p.filter((k) => k !== key)))
  return (
    <Overlay>
      <DialogFrame
        title="Comando Personalizado"
        width={460}
        onCancel={onClose}
        onOk={picked.length ? () => onRun(picked, firstTeam, onlyPerson) : undefined}
        okLabel="OK"
      >
        {failed && (
          <div className="p-2 text-[13px] text-[#ff8080]">
            Não foi possível ler os comandos ({failed}). Se o jogo corre no navegador, reinicie a ponte (web.ps1).
          </div>
        )}
        <div className="max-h-[420px] overflow-auto">
          {list.map((c) => (
            <label key={c.key} className="flex h-6 items-center gap-2 px-1 text-[14px] text-white hover:bg-black/30">
              <input type="checkbox" checked={picked.includes(c.key)} onChange={(e) => toggle(c.key, e.target.checked)} />
              {c.label}
            </label>
          ))}
        </div>
        <div className="mt-2 border-t border-white/15 pt-2">
          <label className="flex h-6 items-center gap-2 px-1 text-[14px] text-cm-yellow">
            <input type="checkbox" checked={firstTeam} onChange={(e) => setFirstTeam(e.target.checked)} />
            Só o plantel principal?
          </label>
          <label className={`flex h-6 items-center gap-2 px-1 text-[14px] ${person ? 'text-cm-yellow' : 'text-[#777]'}`}>
            <input type="checkbox" disabled={!person} checked={onlyPerson} onChange={(e) => setOnlyPerson(e.target.checked)} />
            Só a pessoa actual?{person ? ` (${person})` : ''}
          </label>
          <div className="px-1 pt-1 text-[12px] text-[#9a9aa6]">
            {onlyPerson && person ? `Aplica a ${person}.` : `Aplica aos ${count} jogadores da lista.`}
          </div>
        </div>
      </DialogFrame>
    </Overlay>
  )
}
