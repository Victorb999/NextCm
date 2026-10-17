import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import type { NationOption } from '../bindings/NationOption'
import { AGE_BANDS, EMPTY_FILTERS, POSITION_GROUPS, SIDES, type Filters } from './searchFilters'

function Option({ selected, onClick, children }: { selected: boolean; onClick: () => void; children: ReactNode }) {
  return (
    <button aria-pressed={selected} onClick={onClick} className="cm-option min-w-[86px] py-0.5">
      {children}
    </button>
  )
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex items-center gap-4">
      <span className="w-40 font-title text-[19px] text-white">{label}</span>
      <div className="flex flex-1 items-center gap-3">{children}</div>
    </div>
  )
}

/** Diálogo "Filtros para Pesquisar Jogador", com o fundo de campo verde do CM. */
export default function FilterDialog({
  initial,
  players,
  nations,
  onCancel,
  onOk,
}: {
  initial: Filters
  players: boolean
  nations: NationOption[]
  onCancel: () => void
  onOk: (f: Filters) => void
}) {
  const [f, setF] = useState(initial)
  const set = (patch: Partial<Filters>) => setF((old) => ({ ...old, ...patch }))
  const brazil = nations.find((n) => n.name === 'Brazil')
  const otherNation = f.nationId !== null && f.nationId !== brazil?.id

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onCancel()
      if (e.key === 'Enter') onOk(f)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  })

  return (
    <div className="absolute inset-0 z-20 flex flex-col bg-[#101010]/95 p-4">
      <div className="flex h-12 shrink-0 items-center justify-center border-2 border-[#333] bg-black font-title text-2xl text-white">
        {players ? 'Filtros para Pesquisar Jogador' : 'Filtros para Pesquisar Staff'}
      </div>
      <div className="flex shrink-0 gap-0.5 py-2">
        <button className="cm-purple w-32 border-cm-yellow">Geral</button>
        <button className="cm-purple w-32" disabled title="Em breve">
          Atributos
        </button>
        <div className="flex-1" />
        <button className="cm-purple w-32" onClick={() => setF(EMPTY_FILTERS)}>
          Limpar
        </button>
      </div>
      <div
        className="min-h-0 flex-1 space-y-3 overflow-auto border-2 border-[#1d3a1f] p-4"
        style={{
          background:
            'repeating-linear-gradient(90deg, #0f5a16 0 70px, #126a1a 70px 140px)',
        }}
      >
        <Row label="Texto">
          <Option selected={!f.text} onClick={() => set({ text: '' })}>
            Qualquer
          </Option>
          <input
            autoFocus
            value={f.text}
            onChange={(e) => set({ text: e.target.value })}
            placeholder="Específico"
            className="cm-field w-64 placeholder:text-cm-green"
          />
        </Row>
        <Row label="Nacionalidade">
          <Option selected={f.nationId === null} onClick={() => set({ nationId: null })}>
            Qualquer
          </Option>
          {brazil && (
            <Option selected={f.nationId === brazil.id} onClick={() => set({ nationId: brazil.id })}>
              BRA
            </Option>
          )}
          <select
            value={otherNation ? String(f.nationId) : ''}
            onChange={(e) => set({ nationId: e.target.value ? Number(e.target.value) : null })}
            className={`cm-field ${otherNation ? 'border-cm-yellow' : 'text-cm-green'}`}
          >
            <option value="">Outro…</option>
            {nations.map((n) => (
              <option key={n.id} value={n.id}>
                {n.name}
              </option>
            ))}
          </select>
        </Row>
        <Row label="Idade">
          <Option selected={f.age === null} onClick={() => set({ age: null })}>
            Qualquer
          </Option>
          {AGE_BANDS.map((b) => (
            <Option key={b.key} selected={f.age === b.key} onClick={() => set({ age: b.key })}>
              {b.label}
            </Option>
          ))}
        </Row>
        {players && (
          <>
            <Row label="Posição">
              <Option selected={f.positionGroup === null} onClick={() => set({ positionGroup: null })}>
                Qualquer
              </Option>
              {POSITION_GROUPS.map((g) => (
                <Option key={g.key} selected={f.positionGroup === g.key} onClick={() => set({ positionGroup: g.key })}>
                  {g.label}
                </Option>
              ))}
            </Row>
            <Row label="Lado">
              <Option selected={f.side === null} onClick={() => set({ side: null })}>
                Qualquer
              </Option>
              {SIDES.map((s) => (
                <Option key={s.key} selected={f.side === s.key} onClick={() => set({ side: s.key })}>
                  {s.label}
                </Option>
              ))}
            </Row>
          </>
        )}
      </div>
      <div className="flex h-11 shrink-0 gap-0.5 pt-2">
        <button className="cm-purple flex-[2] font-title text-xl" onClick={onCancel}>
          Cancelar
        </button>
        <button className="cm-purple flex-1 font-title text-xl" onClick={() => onOk(f)}>
          Ok
        </button>
      </div>
    </div>
  )
}
