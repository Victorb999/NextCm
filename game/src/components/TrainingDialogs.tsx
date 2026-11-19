import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import type { TrainingProgram } from '../bindings/TrainingProgram'
import { DialogFrame, DialogRow, Overlay } from './Dialog'

// O diálogo verde da tela de Treinos: o programa (intensidades, nova posição e novo lado).

const INTENSITIES = ['Nenhum', 'Leve', 'Médio', 'Intensivo']

const AREAS: [keyof TrainingProgram, string][] = [
  ['fitness', 'Prep. Física'],
  ['tactics', 'Tática'],
  ['shooting', 'Chute'],
  ['technique', 'Técnica'],
  ['goalkeeping', 'Goleiros'],
]

const POSITIONS: [string, string][] = [
  ['GR', 'Goleiro'],
  ['L', 'Líbero'],
  ['D', 'Zagueiro'],
  ['AL', 'Ala'],
  ['MD', 'Volante'],
  ['M', 'Meio-Campista'],
  ['MA', 'Meia'],
  ['A', 'Atacante'],
]

const SIDES: [string, string][] = [
  ['E', 'Esquerdo'],
  ['C', 'Centro'],
  ['D', 'Direito'],
]
/** Os lados que existem em cada posição: o ala só nas faixas, o guarda-redes e o líbero só ao centro. */
const sidesOf = (position: string | null) =>
  position === 'AL' ? SIDES.filter(([k]) => k !== 'C') : position === 'GR' || position === 'L' ? SIDES.filter(([k]) => k === 'C') : SIDES

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
            <input
              value={p.name}
              maxLength={24}
              onChange={(e) => setP({ ...p, name: e.target.value })}
              className="cm-field w-64"
              autoFocus
            />
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
              onChange={(e) => {
                const newPosition = e.target.value || null
                // Um lado que não existe nesta posição (o "ala central") sai
                const keep = sidesOf(newPosition).some(([k]) => k === p.newSide)
                setP({ ...p, newPosition, newSide: keep ? p.newSide : null })
              }}
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
              {sidesOf(p.newPosition).map(([k, label]) => (
                <option key={k} value={k}>
                  {label}
                </option>
              ))}
            </select>
          </DialogRow>
          <div className="flex items-center gap-3 pt-1">
            <span className="flex-1 text-[12px] text-cm-dlgtext">
              A Prep. Física intensiva cansa e aumenta o risco de lesão; aprender uma posição nova leva meses.
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
