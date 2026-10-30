import { currency, moneyNext, moneyOptions } from '../format'

/**
 * Dinheiro como no CM: uma lista de valores redondos com − / + ao lado. `value` em euros,
 * mostrado na moeda das Definições (× `factor`: o salário no período escolhido).
 */
export default function MoneySelect({ value, onChange, factor = 1 }: { value: number; onChange: (eur: number) => void; factor?: number }) {
  const c = currency()
  const shown = Math.round(value * factor * c.rate)
  const set = (v: number) => onChange(Math.max(0, Math.round(v / factor / c.rate)))
  return (
    <div className="flex items-center gap-1">
      <button className="cm-purple h-7 w-8 px-0" disabled={shown <= 0} onClick={() => set(moneyNext(shown, -1))}>
        −
      </button>
      <select
        value={shown}
        onChange={(e) => set(Number(e.target.value))}
        className="h-7 w-44 rounded border border-[#2f6b52] bg-black px-2 text-right text-[15px] tabular-nums text-cm-yellow outline-none focus:border-cm-yellow"
      >
        {moneyOptions(shown).map((v) => (
          <option key={v} value={v}>
            {c.symbol}
            {v.toLocaleString('pt-BR')}
          </option>
        ))}
      </select>
      <button className="cm-purple h-7 w-8 px-0" onClick={() => set(moneyNext(shown, 1))}>
        +
      </button>
    </div>
  )
}
