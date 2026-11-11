import type { ContractExtras } from '../bindings/ContractExtras'
import { money } from '../format'
import MoneySelect from './MoneySelect'
import { Panel } from './Screen'

// As abas Prémios e Cláusulas do Propôr Contrato, como no CM: cada linha liga-se e desliga-se
// clicando no nome (a roxo quando não entra na proposta); "*" marca o que o jogador exige e o
// valor que ele pede vai a laranja entre parênteses. Os números e as exigências vêm do core.

type Money = 'goalBonus' | 'assistBonus' | 'cleanSheetBonus'
type Flag = 'relegation' | 'nonPromotion' | 'managerJob'

const BONUSES: [Money, string][] = [
  ['goalBonus', 'Prémio por golo marcado'],
  ['assistBonus', 'Prémio por assistência'],
  ['cleanSheetBonus', 'Prémio por jogo sem sofrer golos'],
]

function Row({
  label,
  on,
  asked,
  onToggle,
  children,
}: {
  label: string
  on: boolean
  asked: boolean
  onToggle?: () => void
  children?: React.ReactNode
}) {
  return (
    <div className="flex min-h-9 items-center gap-3 px-4">
      <span className="w-3 shrink-0 font-bold text-cm-orange">{asked ? '*' : ''}</span>
      <button
        onClick={onToggle}
        disabled={!onToggle}
        className={`w-96 shrink-0 text-left text-[15px] font-semibold ${on ? 'text-white' : 'text-cm-purplelight'} enabled:hover:underline`}
        title={onToggle ? (on ? 'Clique para tirar da proposta' : 'Clique para pôr na proposta') : undefined}
      >
        {label}
      </button>
      {children}
    </div>
  )
}

export default function ContractExtrasPanel({
  tab,
  value,
  asks,
  suggested,
  disabled,
  onChange,
}: {
  tab: 'bonuses' | 'clauses'
  value: ContractExtras
  asks: ContractExtras
  suggested: ContractExtras
  disabled: boolean
  onChange: (e: ContractExtras) => void
}) {
  const set = (patch: Partial<ContractExtras>) => onChange({ ...value, ...patch })
  const money_ = (key: Money) => {
    const on = value[key] > 0
    return (
      <Row
        key={key}
        label={BONUSES.find((b) => b[0] === key)![1]}
        on={on}
        asked={asks[key] > 0}
        onToggle={disabled ? undefined : () => set({ [key]: on ? 0 : asks[key] || suggested[key] })}
      >
        {on && (
          <>
            <MoneySelect value={value[key]} onChange={(v) => !disabled && set({ [key]: v })} />
            {asks[key] > 0 && (
              <button className="cm-purple h-7 w-8 px-0" title="O que ele pede" disabled={disabled} onClick={() => set({ [key]: asks[key] })}>
                =
              </button>
            )}
          </>
        )}
        {asks[key] > 0 && <span className="text-[14px] text-cm-orange">({money(asks[key])})</span>}
      </Row>
    )
  }
  const flag = (key: Flag, label: string) => (
    <Row key={key} label={label} on={value[key]} asked={asks[key]} onToggle={disabled ? undefined : () => set({ [key]: !value[key] })} />
  )
  const minFee = value.minFee
  return (
    <Panel title={tab === 'bonuses' ? 'Prémios' : 'Cláusulas'} className="shrink-0">
      <div className="px-4 pt-2 text-[14px] text-cm-muted">
        {tab === 'bonuses' ? 'Propôr ao jogador os seguintes prémios:' : 'Propôr ao jogador as seguintes cláusulas:'}
      </div>
      <div className="cm-zebra py-1">
        {tab === 'bonuses' ? (
          // Sem valor sugerido o prémio não é para ele (o sem sofrer golos é só dos guarda-redes)
          BONUSES.filter(([k]) => suggested[k] > 0 || value[k] > 0).map(([k]) => money_(k))
        ) : (
          <>
            {flag('nonPromotion', 'Cláusula de rescisão (se o clube não subir)')}
            {flag('relegation', 'Cláusula de dispensa por despromoção')}
            <Row
              label="Cláusula de venda"
              on={minFee !== null}
              asked={asks.minFee !== null}
              onToggle={disabled ? undefined : () => set({ minFee: minFee !== null ? null : (asks.minFee ?? suggested.minFee) })}
            >
              {minFee !== null && (
                <>
                  <MoneySelect value={minFee} onChange={(v) => !disabled && set({ minFee: v })} />
                  {asks.minFee !== null && (
                    <button className="cm-purple h-7 w-8 px-0" title="O que ele pede" disabled={disabled} onClick={() => set({ minFee: asks.minFee })}>
                      =
                    </button>
                  )}
                </>
              )}
              {asks.minFee !== null && <span className="text-[14px] text-cm-orange">({money(asks.minFee)})</span>}
            </Row>
          </>
        )}
      </div>
      {tab === 'clauses' && (
        <>
          <div className="px-4 pt-1 text-[14px] text-cm-muted">Cláusulas de Rescisão:</div>
          <div className="cm-zebra py-1">{flag('managerJob', 'Cláusula de dispensa por oferta de lugar de treinador')}</div>
        </>
      )}
      <div className="px-4 pb-2 pt-1 text-[13px] font-semibold text-cm-orange">* Exigido pelo jogador</div>
    </Panel>
  )
}
