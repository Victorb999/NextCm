import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import type { Coach } from '../bindings/Coach'
import type { TrainingProgram } from '../bindings/TrainingProgram'
import { useNav } from '../nav'
import { DialogFrame, DialogRow, Overlay } from './Dialog'
import { Link } from './Screen'

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

/** As áreas do diálogo Preparadores, pela ordem do core. */
const COACH_AREAS = ['Prep. Física', 'Táctica', 'Finalização', 'Técnica', 'Goleiro']

/** Preparadores, como no CM: o que cada treinador do clube treina (moldura amarela nas áreas
 * escolhidas). Grava-se no Ok da tela de Treinos. */
export function CoachesDialog({
  coaches,
  areas: initial,
  onCancel,
  onOk,
}: {
  coaches: Coach[]
  areas: Map<number, boolean[]>
  onCancel: () => void
  onOk: (areas: Map<number, boolean[]>) => void
}) {
  const nav = useNav()
  const [areas, setAreas] = useState(initial)
  const ok = () => onOk(areas)
  useDialogKeys(onCancel, ok)
  const flip = (id: number, i: number) =>
    setAreas((m) => {
      const next = [...(m.get(id) ?? COACH_AREAS.map(() => false))]
      next[i] = !next[i]
      return new Map(m).set(id, next)
    })
  return (
    <Overlay>
      <DialogFrame title="Preparadores" width={760} onCancel={onCancel} onOk={ok}>
        {coaches.length ? (
          <>
            <div className="cm-zebra py-1">
              {coaches.map((c) => (
                <div key={c.id} className="flex h-7 items-center gap-1 px-1 text-[15px]">
                  <span className="min-w-0 flex-1 truncate font-semibold text-white" title={`${c.job} · ${c.works}`}>
                    {/* O perfil do treinador (sai do diálogo e da tela sem gravar) */}
                    <Link onClick={() => nav.go({ kind: 'person', id: c.id })}>{c.name}</Link>
                  </span>
                  <span className="w-20 shrink-0 text-[12px] text-cm-dlgtext" title="Qualidade a treinar">
                    {c.quality}
                  </span>
                  {COACH_AREAS.map((label, i) => (
                    <Option key={label} selected={!!areas.get(c.id)?.[i]} onClick={() => flip(c.id, i)}>
                      {label}
                    </Option>
                  ))}
                </div>
              ))}
            </div>
            <div className="pt-1 text-[12px] text-cm-dlgtext">
              Clique numa área para a dar ou tirar a um treinador. Quem treina muitas áreas rende menos; uma área sem
              ninguém treina-se mal.
            </div>
          </>
        ) : (
          <div className="py-2 text-white">O clube não tem treinadores: o treino rende menos.</div>
        )}
      </DialogFrame>
    </Overlay>
  )
}
