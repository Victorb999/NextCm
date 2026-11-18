import type { ContractExtras } from '../../bindings/ContractExtras'
import type { EditorProfile } from '../../bindings/EditorProfile'
import type { EditorTransferClause } from '../../bindings/EditorTransferClause'
import { BOX, type ClauseKey, ROW } from './editorText'
import { Header, NumField } from './Fields'

/** A caixa Cláusulas da aba Contrato: os prêmios e cláusulas do contrato e os da transferência,
 * todos editáveis. Um clube novo para uma cláusula da transferência vem do `onPick`. */
export default function ClausesBox({
  p,
  setP,
  onPick,
}: {
  p: EditorProfile
  setP: (patch: Partial<EditorProfile>) => void
  onPick: (key: ClauseKey) => void
}) {
  const c = p.clauses
  const e = c.extras
  const setE = (patch: Partial<ContractExtras>) => setP({ clauses: { ...c, extras: { ...e, ...patch } } })
  const setT = (key: ClauseKey, v: EditorTransferClause | null) => setP({ clauses: { ...c, [key]: v } })
  const money = (n: number) => Math.max(0, Math.min(1000000000, n))

  const check = (label: string, on: boolean, change: (v: boolean) => void) => (
    <label className={ROW}>
      <input type="checkbox" checked={on} onChange={(ev) => change(ev.target.checked)} />
      {label}
    </label>
  )

  const transfer = (key: ClauseKey, title: string, amountLabel: string, max: number, games: boolean) => {
    const t = c[key]
    return (
      <div className="border-t border-white/10 pt-0.5">
        <div className={ROW}>
          <span className="flex-1 font-bold text-cm-mint">{title}</span>
          {t ? (
            <button className="cm-purple h-6 px-2 text-[12.5px]" onClick={() => setT(key, null)}>
              Tirar
            </button>
          ) : (
            <button className="cm-purple h-6 px-2 text-[12.5px]" onClick={() => onPick(key)}>
              Adicionar…
            </button>
          )}
        </div>
        {t && (
          <>
            <div className={ROW}>
              <span className="flex-1">Clube que recebe</span>
              <button className="truncate text-cm-magenta hover:underline" title="Mudar o clube" onClick={() => onPick(key)}>
                {t.clubName ?? `#${t.clubId}`}
              </button>
            </div>
            <NumField
              label={amountLabel}
              value={t.amount}
              max={max}
              width="w-32"
              onChange={(v) => setT(key, { ...t, amount: Math.max(games ? 0 : 1, Math.min(max, v)) })}
            />
            {games && (
              <NumField
                label="Jogos que faltam"
                value={t.needed}
                max={500}
                width="w-16"
                onChange={(v) => setT(key, { ...t, needed: Math.max(1, Math.min(500, v)) })}
              />
            )}
          </>
        )}
      </div>
    )
  }

  return (
    <div className={`${BOX} w-[400px]`}>
      <Header>Prêmios</Header>
      <NumField label="Por gol marcado" value={e.goalBonus} width="w-32" onChange={(v) => setE({ goalBonus: money(v) })} />
      <NumField label="Por assistência" value={e.assistBonus} width="w-32" onChange={(v) => setE({ assistBonus: money(v) })} />
      <NumField
        label="Por jogo sem sofrer gols"
        value={e.cleanSheetBonus}
        width="w-32"
        onChange={(v) => setE({ cleanSheetBonus: money(v) })}
      />

      <div className="mt-1.5">
        <Header>Cláusulas do contrato</Header>
      </div>
      <div className={ROW}>
        <input
          type="checkbox"
          checked={e.minFee != null}
          onChange={(ev) => setE({ minFee: ev.target.checked ? (e.minFee ?? 10000000) : null })}
        />
        <span className="min-w-0 flex-1 truncate">Cláusula de rescisão (valor)</span>
        {e.minFee != null && (
          <input
            type="number"
            min={0}
            value={e.minFee}
            onChange={(ev) => setE({ minFee: money(Number(ev.target.value) || 0) })}
            className="cm-field h-6 w-32 text-right text-[14px]"
          />
        )}
      </div>
      {check('Sai livre se o clube for rebaixado', e.relegation, (v) => setE({ relegation: v }))}
      {check('Sai livre se o clube não subir', e.nonPromotion, (v) => setE({ nonPromotion: v }))}
      {check('Sai se lhe oferecerem lugar de treinador', e.managerJob, (v) => setE({ managerJob: v }))}

      <div className="mt-1.5">
        <Header>Cláusulas da transferência</Header>
      </div>
      {transfer('sellOn', 'Venda futura', 'Percentagem da próxima venda (%)', 50, false)}
      {transfer('appsBonus', 'Valor por jogos', 'Valor', 1000000000, true)}
      {transfer('capsBonus', 'Valor por jogos pela seleção', 'Valor', 1000000000, true)}
    </div>
  )
}
