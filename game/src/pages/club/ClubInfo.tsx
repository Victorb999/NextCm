import { useState } from 'react'
import type { ReactNode } from 'react'
import type { ClubProfile } from '../../bindings/ClubProfile'
import FinancesPanel, { type FinanceView } from '../../components/FinancesPanel'
import NationFlag from '../../components/NationFlag'
import { Link, PanelTitle, PurpleMenu } from '../../components/Screen'
import { money } from '../../format'
import { useNav } from '../../nav'
import { financesLabel, INFO_VIEWS } from './squadView'

/** "Finanças & Info" do CM: o Ver ▾ escolhe a Informação Geral (dados do clube e o staff) ou
 * as Finanças, Receitas e Despesas. */
export function Info({ c, mine }: { c: ClubProfile; mine: boolean }) {
  const [view, setView] = useState<'info' | FinanceView>('info')
  const menu = (
    <div className="px-2 pb-1 pt-2">
      <PurpleMenu label="Ver" options={INFO_VIEWS} value={view} onChange={setView} />
    </div>
  )
  if (view !== 'info')
    return (
      <div className="min-h-0 flex-1 overflow-auto pb-3">
        {menu}
        <FinancesPanel clubId={c.club.id} view={view} mine={mine} />
      </div>
    )
  return <GeneralInfo c={c} menu={menu} />
}

function GeneralInfo({ c, menu }: { c: ClubProfile; menu: ReactNode }) {
  const nav = useNav()
  const row = (label: string, value: ReactNode) => (
    <div key={label} className="flex min-h-6 items-baseline gap-4 px-3 py-1">
      <span className="w-56 shrink-0 text-[15px] font-semibold text-white">{label}</span>
      <span className="flex-1 text-[14px] text-cm-yellow">{value}</span>
    </div>
  )
  const competition = c.division?.name
  return (
    <div className="min-h-0 flex-1 overflow-auto pb-3">
      {menu}
      <PanelTitle title="Informação Geral" />
      <div className="cm-zebra">
        {row(
          'País',
          c.nationId !== null && c.nation ? (
            <span className="inline-flex items-center gap-1.5">
              <NationFlag name={c.nation} round />
              <Link onClick={() => nav.go({ kind: 'nation', id: c.nationId! })}>{c.nation}</Link>
            </span>
          ) : (
            '-'
          ),
        )}
        {row('Estatuto', `${c.status ?? '-'}${competition ? ` (${competition})` : ''}`)}
        {row('Finanças', `${financesLabel(c.cash)} · saldo ${money(c.cash)}`)}
        {row('Estádio', [c.stadium, c.stadiumCity].filter(Boolean).join(', ') || '-')}
        {row(
          'Capacidade',
          c.stadiumCapacity
            ? `${c.stadiumCapacity}${c.stadiumSeated === c.stadiumCapacity ? ' - todos lugares sentados' : c.stadiumSeated ? ` - (${c.stadiumSeated} lugares sentados)` : ''}${c.undersoilHeating ? ' com aquecimento subterrâneo' : ''}${c.retractableRoof ? ' e telhado retráctil' : ''}`
            : '-',
        )}
        {c.stadium && c.ownsStadium && row('Propriedade', 'Estádio do clube')}
        {c.attendance && row('Assistência', `média ${c.attendance[0]} (mín. ${c.attendance[1]}, máx. ${c.attendance[2]})`)}
        {c.matchDay && row('Dia de Jogo', c.matchDay)}
        {c.facilities && row('Condições Trabalho', c.facilities)}
        {row('Reputação', c.reputation ?? '-')}
        {c.reserveDivision &&
          row('Estadual', <Link onClick={() => nav.go({ kind: 'competition', id: c.reserveDivision!.id })}>{c.reserveDivision.name}</Link>)}
        {row(
          'Rivais',
          c.rivals.length
            ? c.rivals.map((r, i) => (
                <span key={r.id}>
                  {i > 0 && ', '}
                  <Link onClick={() => nav.go({ kind: 'club', id: r.id })}>{r.shortName}</Link>
                </span>
              ))
            : '-',
        )}
      </div>
      <PanelTitle title="Staff" />
      {/* Uma linha por função, com os nomes (ligações para o perfil) */}
      <div className="cm-zebra">
        {[...new Set(c.staff.map((s) => s.job ?? '-'))].map((job) =>
          row(
            job,
            c.staff
              .filter((s) => (s.job ?? '-') === job)
              .map((s, i) => (
                <span key={s.id}>
                  {i > 0 && ', '}
                  <Link onClick={() => nav.go({ kind: 'person', id: s.id })}>
                    {/* "Sobrenome, Nome" -> "Nome Sobrenome", como na lista do CM */}
                    {s.listName.includes(', ') ? s.listName.split(', ').reverse().join(' ') : s.listName}
                  </Link>
                </span>
              )),
          ),
        )}
      </div>
    </div>
  )
}
