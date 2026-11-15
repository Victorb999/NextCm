import { useState } from 'react'
import type { EditorProfile } from '../../bindings/EditorProfile'
import type { EditorProfileView } from '../../bindings/EditorProfileView'
import { BOX, ROW, TAB } from './editorText'
import { DateField, Header, NumField } from './Fields'
import PickDialog from './PickDialog'

type Sub = 'details' | 'unhappy' | 'club' | 'loan'

/** A aba Contrato do jogador, como no editor do CM: Detalhes do Contrato (salário, datas, listas,
 * função), Insatisfação, Insatisfação com o Clube e Empréstimo. */
export default function ContractTab({
  file,
  view,
  p,
  setP,
}: {
  file: string
  view: EditorProfileView
  p: EditorProfile
  setP: (patch: Partial<EditorProfile>) => void
}) {
  const [sub, setSub] = useState<Sub>('details')
  const [picking, setPicking] = useState(false)
  const check = (label: string, on: boolean, change: (v: boolean) => void) => (
    <label className="flex h-7 items-center gap-2 px-2 text-[14px] text-white">
      <input type="checkbox" checked={on} onChange={(e) => change(e.target.checked)} />
      {label}
    </label>
  )
  /** Termina um ano depois (o "Extend Contract By 1 Year" do CM). */
  const extend = () => {
    const [y, m, d] = (p.contractExpires ?? '').split('-')
    if (y && m && d) setP({ contractExpires: `${Number(y) + 1}-${m}-${d}` })
  }
  const setReasons = (club: boolean, all: boolean) => {
    const keys = view.reasons.filter((r) => r.club === club).map((r) => r.key)
    const rest = p.unhappy.filter((k) => !keys.includes(k))
    setP({ unhappy: all ? [...rest, ...keys] : rest })
  }
  const toggleReason = (key: string, on: boolean) =>
    setP({ unhappy: on ? [...p.unhappy.filter((k) => k !== key), key] : p.unhappy.filter((k) => k !== key) })
  const count = (club: boolean) => view.reasons.filter((r) => r.club === club && p.unhappy.includes(r.key)).length

  const reasonsBox = (club: boolean) => (
    <div className={BOX}>
      <Header>{club ? 'Estado de contentamento com o clube' : 'Estado de insatisfação'}</Header>
      <div className="px-2 pb-1 text-[13px] text-cm-lilac">
        {count(club) ? `Marcados ${count(club)}: é o que o jogador sente agora.` : 'Nenhum marcado: o jogador está contente.'}
      </div>
      {!view.managed && (
        <div className="px-2 pb-1 text-[13px] text-cm-orange">
          A insatisfação só conta no plantel do treinador: noutros clubes o jogo apaga-a no dia seguinte.
        </div>
      )}
      <div className="grid grid-cols-2 gap-x-6">
        {view.reasons
          .filter((r) => r.club === club)
          .map((r) => (
            <label key={r.key} className="flex h-7 items-center gap-2 text-[14px] text-cm-yellow">
              <input type="checkbox" checked={p.unhappy.includes(r.key)} onChange={(e) => toggleReason(r.key, e.target.checked)} />
              {r.label}
            </label>
          ))}
      </div>
      <div className="mt-2 flex justify-end gap-1">
        <button className="cm-purple h-7 w-28 text-[13px]" onClick={() => setReasons(club, true)}>
          Marcar todos
        </button>
        <button className="cm-purple h-7 w-28 text-[13px]" onClick={() => setReasons(club, false)}>
          Limpar todos
        </button>
      </div>
    </div>
  )

  return (
    <>
      <div className="mb-1.5 flex gap-1">
        {(
          [
            ['details', 'Detalhes do Contrato'],
            ['unhappy', `Insatisfação${count(false) ? ` (${count(false)})` : ''}`],
            ['club', `Insatisfação com o Clube${count(true) ? ` (${count(true)})` : ''}`],
            ['loan', `Empréstimo${p.loan ? ' ●' : ''}`],
          ] as [Sub, string][]
        ).map(([s, label]) => (
          <button key={s} className={TAB(sub === s)} onClick={() => setSub(s)}>
            {label}
          </button>
        ))}
      </div>

      {sub === 'details' && (
        <div className="flex items-start gap-2">
          <div className={`${BOX} w-[560px]`}>
            <Header>Detalhes do contrato</Header>
            <NumField label="Salário semanal" value={p.wage} max={100000000} width="w-36" onChange={(v) => setP({ wage: v })} />
            <DateField label="Início" value={p.contractStart} onChange={(v) => setP({ contractStart: v })} />
            <DateField label="Termina" value={p.contractExpires} onChange={(v) => setP({ contractExpires: v })} />
            <div className="flex justify-end px-2 py-1">
              <button className="cm-purple h-7 px-3 text-[13px]" disabled={!p.contractExpires} onClick={extend}>
                Prolongar contrato 1 ano
              </button>
            </div>
            {check('Na lista de transferências pelo clube', p.listed, (v) => setP({ listed: v }))}
            {check('Na lista de transferências a pedido dele', p.requested, (v) => setP({ requested: v }))}
            {check('Na lista de empréstimos', p.loanListed, (v) => setP({ loanListed: v }))}
            <label className={ROW}>
              <span className="w-20 shrink-0">Função</span>
              <select
                className="cm-field h-6 min-w-0 flex-1 text-[13.5px]"
                value={p.role ?? ''}
                onChange={(e) => setP({ role: e.target.value || null })}
              >
                <option value="">— Pelo lugar no plantel —</option>
                {view.roles.map((r) => (
                  <option key={r.key} value={r.key}>
                    {r.label}
                  </option>
                ))}
              </select>
            </label>
          </div>
          {p.missed != null && (
            <div className={`${BOX} w-64`}>
              <Header>Diversos</Header>
              <NumField
                label="Jogos sem jogar seguidos"
                value={p.missed}
                max={99}
                width="w-14"
                onChange={(v) => setP({ missed: Math.min(99, Math.max(0, v)) })}
              />
            </div>
          )}
        </div>
      )}
      {sub === 'unhappy' && reasonsBox(false)}
      {sub === 'club' && reasonsBox(true)}
      {sub === 'loan' && (
        <div className={`${BOX} w-[560px]`}>
          <Header>Contrato de empréstimo</Header>
          {p.loan ? (
            <>
              <div className={ROW}>
                <span className="flex-1">Clube dono do passe</span>
                <span className="truncate text-cm-magenta">{p.loan.parentName ?? `#${p.loan.parentId}`}</span>
              </div>
              <DateField label="Até" value={p.loan.until} onChange={(v) => p.loan && setP({ loan: { ...p.loan, until: v ?? '' } })} />
              <NumField
                label="Parte do salário paga por quem o tem (%)"
                value={p.loan.wageShare}
                max={100}
                width="w-16"
                onChange={(v) => p.loan && setP({ loan: { ...p.loan, wageShare: Math.min(100, Math.max(0, v)) } })}
              />
              <div className="mt-1 flex justify-end gap-1">
                <button className="cm-purple h-7 px-3 text-[13px]" onClick={() => setPicking(true)}>
                  Mudar o clube dono…
                </button>
                <button className="cm-purple h-7 px-3 text-[13px]" onClick={() => setP({ loan: null })}>
                  Acabar o empréstimo (fica no clube)
                </button>
              </div>
            </>
          ) : (
            <>
              <div className="px-2 py-1 text-[14px] text-cm-lilac">Não está emprestado.</div>
              <div className="flex justify-end">
                <button className="cm-purple h-7 px-3 text-[13px]" onClick={() => setPicking(true)}>
                  Emprestado por outro clube…
                </button>
              </div>
            </>
          )}
        </div>
      )}
      {picking && (
        <PickDialog
          file={file}
          kind="club"
          title="Clube Dono do Passe"
          onClose={() => setPicking(false)}
          onPick={(h) => {
            setPicking(false)
            setP({
              loan: {
                parentId: h.id,
                parentName: h.name,
                until: p.loan?.until ?? p.contractExpires ?? '',
                wageShare: p.loan?.wageShare ?? 100,
              },
            })
          }}
        />
      )}
    </>
  )
}
