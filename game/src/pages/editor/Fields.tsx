// Campos do Editor de Jogos Salvos: atributo, número, cor, data, nação e o cabeçalho das caixas.

import type { EditorAttribute } from '../../bindings/EditorAttribute'
import { LABELS, ROW } from './editorText'

export function AttributeRow({ a, value, onChange }: { a: EditorAttribute; value: number; onChange: (v: number) => void }) {
  const changed = value !== a.shown
  return (
    <div
      className={`flex h-7 items-center gap-2 px-2 text-[14px] ${a.hidden ? 'rounded bg-white/90 text-black' : 'even:bg-black/20 text-white'}`}
    >
      <span className="min-w-0 flex-1 truncate">{LABELS[a.key] ?? a.key}</span>
      <input
        type="number"
        min={1}
        max={20}
        value={value}
        onChange={(e) => onChange(Math.min(20, Math.max(1, Number(e.target.value) || 1)))}
        className={`cm-field h-6 w-14 text-center text-[14px] ${changed ? '!text-cm-yellow' : ''}`}
      />
      <span className="w-10 text-right font-mono text-[12px] text-[#8a8a8a]" title="Valor guardado no save (só leitura)">
        {a.relative ? a.stored : ''}
      </span>
    </div>
  )
}

export function NumField({
  label,
  value,
  onChange,
  min = 0,
  max = 999999999,
  width = 'w-28',
}: {
  label: string
  value: number
  onChange: (v: number) => void
  min?: number
  max?: number
  width?: string
}) {
  return (
    <label className={ROW}>
      <span className="min-w-0 flex-1 truncate">{label}</span>
      <input
        type="number"
        min={min}
        max={max}
        value={value}
        onChange={(e) => onChange(Number(e.target.value) || 0)}
        className={`cm-field h-6 ${width} text-right text-[14px]`}
      />
    </label>
  )
}

export function ColorField({ label, value, onChange }: { label: string; value: string | null; onChange: (v: string) => void }) {
  return (
    <label className={ROW}>
      <span className="min-w-0 flex-1 truncate">{label}</span>
      <input
        value={value ?? ''}
        placeholder="#rrggbb"
        onChange={(e) => onChange(e.target.value)}
        className="cm-field h-6 w-24 text-center font-mono text-[13px]"
      />
      <span className="h-5 w-12 rounded border border-black/60" style={{ background: value || 'transparent' }} />
    </label>
  )
}

export function Header({ children }: { children: React.ReactNode }) {
  return (
    <div className="mb-0.5 bg-cm-headdeep px-2 font-mono text-[11px] font-bold uppercase leading-6 tracking-[0.1em] text-cm-yellow">
      {children}
    </div>
  )
}

/** Data em três caixas (dia, mês, ano) como no editor do CM; devolve "AAAA-MM-DD" (o core valida). */
export function DateField({ label, value, onChange }: { label: string; value: string | null; onChange: (v: string | null) => void }) {
  const [y = '', m = '', d = ''] = (value ?? '').split('-')
  const set = (yy: string, mm: string, dd: string) =>
    onChange(yy || mm || dd ? `${yy}-${mm.padStart(2, '0')}-${dd.padStart(2, '0')}` : null)
  const box = 'cm-field h-6 text-center text-[14px]'
  return (
    <div className={ROW}>
      <span className="min-w-0 flex-1 truncate">{label}</span>
      <input
        className={`${box} w-12`}
        inputMode="numeric"
        maxLength={2}
        value={d === '' ? '' : String(Number(d))}
        placeholder="dd"
        onChange={(e) => set(y, m, e.target.value.replace(/\D/g, ''))}
      />
      <input
        className={`${box} w-12`}
        inputMode="numeric"
        maxLength={2}
        value={m === '' ? '' : String(Number(m))}
        placeholder="mm"
        onChange={(e) => set(y, e.target.value.replace(/\D/g, ''), d)}
      />
      <input
        className={`${box} w-20`}
        inputMode="numeric"
        maxLength={4}
        value={y}
        placeholder="aaaa"
        onChange={(e) => set(e.target.value.replace(/\D/g, ''), m, d)}
      />
    </div>
  )
}

export function NationSelect({
  label,
  value,
  nations,
  onChange,
}: {
  label: string
  value: number | null
  nations: { id: number; name: string }[]
  onChange: (v: number | null) => void
}) {
  return (
    <label className={ROW}>
      <span className="w-24">{label}</span>
      <select
        className="cm-field h-6 min-w-0 flex-1 text-[14px]"
        value={value ?? ''}
        onChange={(e) => onChange(e.target.value ? Number(e.target.value) : null)}
      >
        <option value="">—</option>
        {nations.map((n) => (
          <option key={n.id} value={n.id}>
            {n.name}
          </option>
        ))}
      </select>
    </label>
  )
}
