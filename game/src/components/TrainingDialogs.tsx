import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import type { Coach } from '../bindings/Coach'
import type { TrainingProgram } from '../bindings/TrainingProgram'
import { DialogFrame, DialogRow, Overlay } from './Dialog'

// Os diálogos verdes da tela de Treinos: o programa (intensidades, nova posição e novo lado)
// e os treinadores do clube.

const INTENSITIES = ['Nenhum', 'Ligeiro', 'Médio', 'Intensivo']

const AREAS: [keyof TrainingProgram, string][] = [
  ['fitness', 'Prep. Física'],
  ['tactics', 'Táctica'],
  ['shooting', 'Remate'],
  ['technique', 'Técnica'],
  ['goalkeeping', 'G-Redes'],
]

const POSITIONS: [string, string][] = [
  ['GR', 'Guarda-Redes'],
  ['L', 'Líbero'],
  ['D', 'Defesa'],
  ['AL', 'Ala'],
  ['MD', 'Médio Defensivo'],
  ['M', 'Médio'],
  ['MA', 'Médio Atacante'],
  ['A', 'Avançado'],
]

const SIDES: [string, string][] = [
  ['E', 'Esquerdo'],
  ['C', 'Centro'],
  ['D', 'Direito'],
]

function Option({ selected, onClick, children }: { selected: boolean; onClick: () => void; children: ReactNode }) {
  return (
    <button aria-pressed={selected} onClick={onClick} className="cm-option min-w-[86px] py-0.5">
      {children}
    </button>
  )
}

/** Fecha com Esc (e Enter confirma), como os outros diálogos verdes. */
function useDialogKeys(onCancel: () => void, onOk?: () => void) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopImmediatePropagation()
        onCancel()
      }
      if (e.key === 'Enter' && onOk && !(document.activeElement instanceof HTMLInputElement)) onOk()
    }
    window.addEventListener('keydown', onKey, true)
    return () => window.removeEventListener('keydown', onKey, true)
  })
}

/** O programa de treino: nome, intensidade por área, nova posição e novo lado. */
export function ProgramDialog({
  initial,
  canDelete,
  onCancel,
  onOk,
  onDelete,
}: {
  initial: TrainingProgram
  canDelete: boolean
  onCancel: () => void
  onOk: (p: TrainingProgram) => void
  onDelete: () => void
}) {
  const [p, setP] = useState(initial)
  const ok = p.name.trim() ? () => onOk({ ...p, name: p.name.trim() }) : undefined
  useDialogKeys(onCancel, ok)
  return (
    <Overlay>
      <DialogFrame title="Programa de Treino" width={680} onCancel={onCancel} onOk={ok}>
        <div className="space-y-1 py-1">
          <DialogRow label="Nome">
            <input value={p.name} maxLength={24} onChange={(e) => setP({ ...p, name: e.target.value })} className="cm-field w-64" autoFocus />
          </DialogRow>
          {AREAS.map(([key, label]) => (
            <DialogRow key={key} label={label}>
              {INTENSITIES.map((name, i) => (
                <Option key={name} selected={p[key] === i} onClick={() => setP({ ...p, [key]: i })}>
                  {name}
                </Option>
              ))}
            </DialogRow>
          ))}
          <DialogRow label="Nova Posição">
            <select
              value={p.newPosition ?? ''}
              onChange={(e) => setP({ ...p, newPosition: e.target.value || null })}
              className="cm-field min-w-56"
            >
              <option value="">Nenhuma</option>
              {POSITIONS.map(([k, label]) => (
                <option key={k} value={k}>
                  {label}
                </option>
              ))}
            </select>
          </DialogRow>
          <DialogRow label="Novo Lado">
            <select value={p.newSide ?? ''} onChange={(e) => setP({ ...p, newSide: e.target.value || null })} className="cm-field min-w-56">
              <option value="">Nenhum</option>
              {SIDES.map(([k, label]) => (
                <option key={k} value={k}>
                  {label}
                </option>
              ))}
            </select>
          </DialogRow>
          <div className="flex items-center gap-3 pt-1">
            <span className="flex-1 text-[12px] text-[#bfe6c4]">
              A Prep. Física intensiva cansa e aumenta o risco de lesão; uma posição nova aprende-se em meses.
            </span>
            {canDelete && (
              <button className="cm-purple h-7 px-3" onClick={onDelete}>
                Apagar Programa
              </button>
            )}
          </div>
        </div>
      </DialogFrame>
    </Overlay>
  )
}

/** Os treinadores do clube: função, o que treinam e a qualidade. */
export function CoachesDialog({ coaches, onClose }: { coaches: Coach[]; onClose: () => void }) {
  useDialogKeys(onClose, onClose)
  return (
    <Overlay>
      <DialogFrame title="Treinadores" width={640} onOk={onClose}>
        {coaches.length ? (
          <div className="cm-zebra py-1">
            {coaches.map((c) => (
              <div key={c.id} className="flex h-7 items-center gap-3 px-1 text-[15px]">
                <span className="min-w-0 flex-1 truncate font-semibold text-white">{c.name}</span>
                <span className="w-40 shrink-0 text-[#bfe6c4]">{c.job}</span>
                <span className="w-40 shrink-0 text-cm-yellow">{c.works}</span>
                <span className="w-24 shrink-0 text-right text-white">{c.quality}</span>
              </div>
            ))}
          </div>
        ) : (
          <div className="py-2 text-white">O clube não tem treinadores: o treino rende menos.</div>
        )}
      </DialogFrame>
    </Overlay>
  )
}
