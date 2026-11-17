import { useState } from 'react'
import type { EditorFilter } from '../../bindings/EditorFilter'
import type { EditorOptions } from '../../bindings/EditorOptions'
import { EMPTY_FILTER } from './editorEdits'
import { currency, fromEuros, toEuros, type Kind } from './editorList'
import { BOX, LABELS, POS_LEFT, POS_RIGHT, POSITION_LABELS, TAB } from './editorText'
import { Header } from './Fields'

type Sub = 'general' | 'contract' | 'attr1' | 'attr2'
type Key = keyof EditorFilter

const num = (v: string) => (v.trim() === '' ? null : Math.max(0, Number(v.replace(/\D/g, '')) || 0))
const LINE = 'flex h-7 items-center gap-2 text-[14px] text-white'

/** A aba Procurar do editor, como o Player Search do CM Scout: o nome e, nos jogadores, as abas
 * Geral, Contrato, Atributos 1 e Atributos 2. O core filtra; aqui só se escrevem os filtros. */
export default function SearchForm({
  kind,
  filter,
  options,
  busy,
  onChange,
  onSearch,
}: {
  kind: Kind
  filter: EditorFilter
  options: EditorOptions | null
  busy: boolean
  onChange: (f: EditorFilter) => void
  onSearch: () => void
}) {
  const [sub, setSub] = useState<Sub>('general')
  const set = (patch: Partial<EditorFilter>) => onChange({ ...filter, ...patch })
  const players = kind === 'players'
  const cur = currency()

  /** "Rótulo [mín] – [máx]"; `money` = em milhares da moeda (o filtro guarda euros). */
  const range = (label: string, min: Key, max: Key, top: number | string, money = false) => {
    const read = (k: Key) => {
      const v = filter[k] as number | null
      return (money ? fromEuros(v) : v) ?? ''
    }
    const write = (k: Key, s: string) => set({ [k]: money ? toEuros(num(s)) : num(s) })
    const box = `cm-field h-6 ${money ? 'w-24' : 'w-16'} text-center text-[13.5px]`
    return (
      <div className={LINE}>
        <span className="min-w-0 flex-1 truncate">{label}</span>
        <input className={box} inputMode="numeric" placeholder="0" value={read(min)} onChange={(e) => write(min, e.target.value)} />
        <span className="text-cm-lilac">–</span>
        <input
          className={box}
          inputMode="numeric"
          placeholder={String(top)}
          value={read(max)}
          onChange={(e) => write(max, e.target.value)}
        />
      </div>
    )
  }

  const select = (
    label: string,
    value: string,
    list: { id: number | string; name: string }[],
    change: (v: string) => void,
    any = 'Todas',
  ) => (
    <label className={LINE}>
      <span className="w-36 shrink-0">{label}</span>
      <select className="cm-field h-6 min-w-0 flex-1 text-[13.5px]" value={value} onChange={(e) => change(e.target.value)}>
        <option value="">{any}</option>
        {list.map((o) => (
          <option key={o.id} value={o.id}>
            {o.name}
          </option>
        ))}
      </select>
    </label>
  )
  const idOf = (v: string) => (v ? Number(v) : null)

  const togglePos = (key: string, on: boolean) =>
    set({ positions: on ? [...filter.positions.filter((p) => p !== key), key] : filter.positions.filter((p) => p !== key) })

  /** Os mínimos de uma aba de atributos (1 a 20, como o jogo os mostra). */
  const attrGrid = (keys: string[], cols: string) => {
    const min = (k: string) => filter.attributes.find((a) => a.key === k)?.min ?? null
    const setMin = (k: string, v: number | null) => {
      const rest = filter.attributes.filter((a) => a.key !== k)
      set({ attributes: v && v > 1 ? [...rest, { key: k, min: Math.min(20, v) }] : rest })
    }
    const sorted = [...keys].sort((a, b) => (LABELS[a] ?? a).localeCompare(LABELS[b] ?? b, 'pt'))
    return (
      <div className={BOX}>
        <Header>Mínimo de cada atributo (1–20, como o jogo mostra)</Header>
        <div className={`grid ${cols} gap-x-6`}>
          {sorted.map((k) => (
            <label key={k} className={`${LINE} even:bg-black/20 px-2`}>
              <span className="min-w-0 flex-1 truncate">{LABELS[k] ?? k}</span>
              <input
                className={`cm-field h-6 w-12 text-center text-[13.5px] ${min(k) ? '!text-cm-yellow' : ''}`}
                inputMode="numeric"
                placeholder="–"
                value={min(k) ?? ''}
                onChange={(e) => setMin(k, num(e.target.value))}
              />
            </label>
          ))}
        </div>
        <div className="mt-2 flex justify-end">
          <button
            className="cm-purple h-7 px-3 text-[13px]"
            onClick={() => set({ attributes: filter.attributes.filter((a) => !keys.includes(a.key)) })}
          >
            Limpar esta aba
          </button>
        </div>
      </div>
    )
  }

  const attrCount = (keys: string[]) => filter.attributes.filter((a) => keys.includes(a.key)).length
  const playing = options?.attributes ?? []
  const mental = options?.mental ?? []

  return (
    <form
      className="flex max-w-[980px] flex-col gap-2"
      onSubmit={(e) => {
        e.preventDefault()
        onSearch()
      }}
    >
      <label className={LINE}>
        <span className="w-36 shrink-0">Nome</span>
        <input
          className="cm-field h-7 min-w-0 flex-1 text-[14px]"
          autoFocus
          placeholder={
            kind === 'players'
              ? 'Parte do nome (vazio = todos)'
              : kind === 'staff'
                ? 'Treinador, olheiro, médico… (vazio = todos)'
                : 'Parte do nome do clube (vazio = todos)'
          }
          value={filter.query}
          onChange={(e) => set({ query: e.target.value })}
        />
      </label>

      {players && (
        <>
          <div className="flex gap-1">
            {(
              [
                ['general', 'Geral'],
                ['contract', 'Contrato'],
                ['attr1', `Atributos 1${attrCount(playing) ? ` (${attrCount(playing)})` : ''}`],
                ['attr2', `Atributos 2${attrCount(mental) ? ` (${attrCount(mental)})` : ''}`],
              ] as [Sub, string][]
            ).map(([s, label]) => (
              <button key={s} type="button" className={TAB(sub === s)} onClick={() => setSub(s)}>
                {label}
              </button>
            ))}
          </div>

          {sub === 'general' && (
            <div className="flex items-start gap-2">
              <div className="flex min-w-0 flex-1 flex-col gap-2">
                <div className={BOX}>
                  <Header>Geral</Header>
                  {range('Idade', 'minAge', 'maxAge', 99)}
                  {select('Nação', String(filter.nationId ?? ''), options?.nations ?? [], (v) => set({ nationId: idOf(v) }))}
                  {select(
                    'Joga em (país)',
                    String(filter.basedId ?? ''),
                    options?.nations ?? [],
                    (v) => set({ basedId: idOf(v) }),
                    'Todos',
                  )}
                  {select('Divisão', String(filter.divisionId ?? ''), options?.divisions ?? [], (v) => set({ divisionId: idOf(v) }))}
                </div>
                <div className={BOX}>
                  <Header>Posições (joga lá com 15 ou mais)</Header>
                  <div className="flex gap-6">
                    {[POS_LEFT, POS_RIGHT].map((col, i) => (
                      <div key={i} className={i ? 'w-36' : 'grid flex-1 grid-cols-2 gap-x-4'}>
                        {col.map((k) => (
                          <label key={k} className={LINE}>
                            <input
                              type="checkbox"
                              checked={filter.positions.includes(k)}
                              onChange={(e) => togglePos(k, e.target.checked)}
                            />
                            {POSITION_LABELS[k] ?? k}
                          </label>
                        ))}
                      </div>
                    ))}
                  </div>
                  <div className="pt-1 text-[12px] text-cm-lilac">
                    Basta uma das funções marcadas e, se marcar lados, um dos lados (Meia + Centro = MA C).
                  </div>
                </div>
              </div>
              <div className="flex min-w-0 flex-1 flex-col gap-2">
                <div className={BOX}>
                  <Header>Habilidade</Header>
                  {range('Capacidade atual (CA)', 'minAbility', 'maxAbility', 200)}
                  {range('Potencial (PA)', 'minPotential', 'maxPotential', 200)}
                </div>
                <div className={BOX}>
                  <Header>Reputação</Header>
                  {range('Local', 'minHomeReputation', 'maxHomeReputation', 10000)}
                  {range('Atual', 'minReputation', 'maxReputation', 10000)}
                  {range('Mundial', 'minWorldReputation', 'maxWorldReputation', 10000)}
                </div>
              </div>
            </div>
          )}

          {sub === 'contract' && (
            <div className="flex items-start gap-2">
              <div className="flex min-w-0 flex-1 flex-col gap-2">
                <div className={BOX}>
                  <Header>Contrato</Header>
                  {select(
                    'Clube',
                    filter.clubStatus ?? '',
                    [
                      { id: 'club', name: 'Com clube' },
                      { id: 'free', name: 'Sem clube (livres)' },
                    ],
                    (v) => set({ clubStatus: v || null }),
                    'Todos',
                  )}
                  {range('Contrato até (ano)', 'minExpires', 'maxExpires', 2040)}
                  {select(
                    'Empréstimo',
                    filter.onLoan == null ? '' : filter.onLoan ? 'yes' : 'no',
                    [
                      { id: 'yes', name: 'Só os emprestados' },
                      { id: 'no', name: 'Sem os emprestados' },
                    ],
                    (v) => set({ onLoan: v ? v === 'yes' : null }),
                    'Todos',
                  )}
                </div>
                <div className={BOX}>
                  <Header>Situação</Header>
                  {select(
                    'Listas',
                    filter.listing ?? '',
                    [
                      { id: 'transfer', name: 'Na lista de transferências' },
                      { id: 'requested', name: 'Pediu para sair' },
                      { id: 'loan', name: 'Na lista de empréstimos' },
                      { id: 'none', name: 'Em lista nenhuma' },
                    ],
                    (v) => set({ listing: v || null }),
                  )}
                  {select(
                    'Função no elenco',
                    filter.role ?? '',
                    [{ id: 'none', name: 'Pelo lugar no elenco' }, ...(options?.roles ?? []).map((r) => ({ id: r.key, name: r.label }))],
                    (v) => set({ role: v || null }),
                  )}
                </div>
              </div>
              <div className="flex min-w-0 flex-1 flex-col gap-2">
                <div className={BOX}>
                  <Header>Dinheiro (em milhares de {cur.symbol})</Header>
                  {range('Valor de mercado', 'minValue', 'maxValue', '∞', true)}
                  {range('Salário semanal', 'minWage', 'maxWage', '∞', true)}
                </div>
              </div>
            </div>
          )}

          {sub === 'attr1' && attrGrid(playing, 'grid-cols-3')}
          {sub === 'attr2' && attrGrid(mental, 'grid-cols-2 max-w-xl')}
        </>
      )}

      <div className="flex gap-1">
        <span className="flex-1" />
        {players && (
          <button type="button" className="cm-purple h-8 w-36" onClick={() => onChange(EMPTY_FILTER)}>
            Limpar filtros
          </button>
        )}
        <button className="cm-purple h-8 w-36 text-cm-yellow" disabled={busy}>
          Procurar
        </button>
      </div>
    </form>
  )
}
