import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import type { NationOption } from '../bindings/NationOption'
import { AGE_BANDS, EMPTY_FILTERS, POSITION_GROUPS, SIDES, type Filters } from './searchFilters'

function Option({ selected, onClick, children }: { selected: boolean; onClick: () => void; children: ReactNode }) {
  return (
    <button
      aria-pressed={selected}
      onClick={onClick}
      className={`h-8 min-w-[64px] rounded-[4px] border px-3 text-[13px] ${
        selected ? 'border-cm-yellow bg-[#1d4d3a] font-bold text-cm-yellow' : 'border-[#2f6b52] text-[#d6efe4] hover:bg-[#1d4d3a]/60'
      }`}
    >
      {children}
    </button>
  )
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div>
      <div className="mb-1.5 text-[11px] font-bold uppercase tracking-[0.12em] text-[#4edea3]">{label}</div>
      <div className="flex flex-wrap items-center gap-1.5">{children}</div>
    </div>
  )
}

/** Diálogo "Filtros para Pesquisar Jogador": o painel verde do CM, aberto ao lado da tabela. */
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
    <div className="absolute bottom-[60px] right-2.5 top-[72px] z-30 flex w-[480px] flex-col overflow-hidden rounded-lg border-2 border-[#10b981] bg-[#0f241b] shadow-[0_12px_32px_rgba(0,0,0,0.7)]">
      <div className="flex h-12 shrink-0 items-center gap-2 border-b border-[#2f6b52] px-4">
        <span className="size-2 rounded-full bg-[#4edea3]" />
        <span className="font-bold uppercase tracking-[0.12em] text-white">{players ? 'Filtros para Pesquisar Jogador' : 'Filtros para Pesquisar Staff'}</span>
      </div>
      <div className="flex shrink-0 gap-1.5 px-4 pt-3">
        <button className="cm-purple h-7 w-28 ring-1 ring-cm-yellow">Geral</button>
        <button className="cm-purple h-7 w-28" disabled title="Em breve">
          Atributos
        </button>
        <div className="flex-1" />
        <button className="cm-purple h-7 w-28" onClick={() => setF(EMPTY_FILTERS)}>
          Limpar
        </button>
      </div>
      <div className="min-h-0 flex-1 space-y-4 overflow-auto p-4">
        <Row label="Texto">
          <Option selected={!f.text} onClick={() => set({ text: '' })}>
            Qualquer
          </Option>
          <input
            autoFocus
            value={f.text}
            onChange={(e) => set({ text: e.target.value })}
            placeholder="Específico"
            className="h-8 w-56 rounded-[4px] border border-[#2f6b52] bg-[#0b1a14] px-2.5 text-cm-yellow outline-none placeholder:text-[#4edea3]/60 focus:border-cm-yellow"
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
            className={`h-8 rounded-[4px] border bg-[#0b1a14] px-2 outline-none ${otherNation ? 'border-cm-yellow text-cm-yellow' : 'border-[#2f6b52] text-[#d6efe4]'}`}
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
            <Row label="Contrato">
              <Option selected={!f.expiring} onClick={() => set({ expiring: false })}>
                Qualquer
              </Option>
              <Option selected={f.expiring} onClick={() => set({ expiring: true })}>
                A expirar
              </Option>
            </Row>
            <Row label="Lesões">
              <Option selected={f.injured === null} onClick={() => set({ injured: null })}>
                Qualquer
              </Option>
              <Option selected={f.injured === 'only'} onClick={() => set({ injured: 'only' })}>
                Lesionados
              </Option>
              <Option selected={f.injured === 'exclude'} onClick={() => set({ injured: 'exclude' })}>
                Sem lesionados
              </Option>
            </Row>
            <Row label="Interessados">
              <Option selected={!f.interested} onClick={() => set({ interested: false })}>
                Qualquer
              </Option>
              <Option selected={f.interested} onClick={() => set({ interested: true })}>
                Apenas
              </Option>
            </Row>
            <Row label="Incluir Plantel">
              <Option selected={f.ownSquad} onClick={() => set({ ownSquad: true })}>
                Sim
              </Option>
              <Option selected={!f.ownSquad} onClick={() => set({ ownSquad: false })}>
                Não
              </Option>
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
      <div className="flex shrink-0 justify-end gap-2 border-t border-[#2f6b52] p-3">
        <button className="h-9 w-32 rounded-[4px] border border-white/20 bg-black/30 font-semibold text-white hover:bg-black/50" onClick={onCancel}>
          Cancelar
        </button>
        <button className="h-9 w-32 rounded-[4px] bg-[#10b981] font-bold text-[#06281b] hover:bg-[#4edea3]" onClick={() => onOk(f)}>
          Ok
        </button>
      </div>
    </div>
  )
}
