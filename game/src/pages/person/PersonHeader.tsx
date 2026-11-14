import { useState } from 'react'
import type { ReactNode } from 'react'
import { api } from '../../api'
import type { PersonProfile } from '../../bindings/PersonProfile'
import CascadeMenu, { type MenuNode } from '../../components/CascadeMenu'
import NationFlag from '../../components/NationFlag'
import { Link } from '../../components/Screen'
import { cmDate, money, shortListName, wage } from '../../format'
import { useNav } from '../../nav'

const DEFAULT_COLOURS = { fg: '#ffffff', bg: '#3a3a3a' }

/** Cabeçalho do perfil: número e clube, nome, a linha dos dados, a faixa da posição e as abas,
 * tudo num painel; Acção no canto. */
export default function PersonHeader({ p, action, tabs }: { p: PersonProfile | null; action: ReactNode; tabs: ReactNode }) {
  const nav = useNav()
  const club = p?.club
  const colours = club?.fg && club?.bg ? { fg: club.fg, bg: club.bg } : DEFAULT_COLOURS
  const pl = p?.player
  const born = p?.birthDate ? cmDate(p.birthDate) : p?.birthYear ? String(p.birthYear) : '?'
  const dot = <span className="text-[#5a5a66]"> · </span>
  // O ▸ do canto, como no CM: os outros jogadores do plantel do clube, por ordem alfabética
  const [squadMenu, setSquadMenu] = useState<{ x: number; y: number; items: MenuNode[] } | null>(null)
  const openSquadMenu = (e: React.MouseEvent) => {
    if (squadMenu || !club) {
      setSquadMenu(null)
      return
    }
    const r = e.currentTarget.getBoundingClientRect()
    api.getClub(club.id).then(
      (c) =>
        setSquadMenu({
          x: r.left,
          y: r.bottom + 2,
          items: c.players
            .filter((s) => !s.loanedOut)
            .sort((a, b) => a.listName.localeCompare(b.listName))
            .map((s) => ({ label: shortListName(s.listName), onSelect: () => nav.go({ kind: 'person', id: s.id }) })),
        }),
      () => setSquadMenu(null),
    )
  }
  return (
    <div className="m-2 shrink-0 rounded-md bg-cm-surface px-3 pb-1.5 pt-2">
      <div className="flex items-start gap-3">
        {club && (
          <button
            data-menu-anchor
            className="cm-purple flex h-[44px] w-6 shrink-0 items-center justify-center text-[13px]"
            title="Plantel do clube"
            aria-expanded={!!squadMenu}
            onClick={openSquadMenu}
          >
            ▸
          </button>
        )}
        <div
          className="flex size-[44px] shrink-0 flex-col items-center justify-center rounded border border-black/40 leading-none"
          style={{ color: colours.fg, background: colours.bg }}
          title={club?.name}
        >
          <span className="font-title text-[22px] font-bold">{pl?.squadNumber ?? '-'}</span>
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="truncate font-title text-[22px] font-bold uppercase leading-tight text-white">{p?.name ?? ''}</span>
            {p && (
              <span
                className="shrink-0 rounded px-1.5 font-mono text-[11px] font-bold uppercase tracking-wider"
                style={{ color: colours.fg, background: colours.bg }}
              >
                {club ? (
                  <Link onClick={() => nav.go({ kind: 'club', id: club.id, reserves: p.status?.reserve })}>
                    {p.status?.reserve ? `Reservas ${club.shortName}` : club.shortName}
                  </Link>
                ) : (
                  'Sem clube'
                )}
              </span>
            )}
            {p?.ghost && (
              <span className="shrink-0 rounded bg-cm-headdeep px-1.5 font-mono text-[11px] font-bold uppercase tracking-wider text-cm-purplelight">
                Enchimento
              </span>
            )}
            {p && !pl && p.job && (
              <span className="shrink-0 rounded bg-cm-dlgdeep px-1.5 font-mono text-[11px] font-bold uppercase tracking-wider text-cm-mint">
                {p.job}
              </span>
            )}
          </div>
          {p && (
            <div className="truncate font-mono text-[13px] text-cm-muted">
              {born} ({p.age ?? '?'} anos)
              {dot}
              <NationFlag name={p.nation} round className="mr-1.5" />
              {p.nationId !== null && p.nation ? (
                <Link onClick={() => nav.go({ kind: 'nation', id: p.nationId! })}>{p.nation}</Link>
              ) : (
                (p.nation ?? '-')
              )}
              {p.internationalApps > 0 && (
                <>
                  {' '}
                  (<b className="text-cm-yellow">{p.internationalApps}</b> internac. /{' '}
                  <b className="text-cm-yellow">{p.internationalGoals}</b> golos)
                </>
              )}
              {pl && !p.ghost && (
                <>
                  {dot}Valor: <b className="text-cm-mint">{money(pl.value) || '-'}</b>
                </>
              )}
              {!p.ghost && (
                <>
                  {dot}Salário: <b className="text-white">{wage(p.wage) || '-'}</b>
                </>
              )}
            </div>
          )}
        </div>
        <div className="shrink-0">{action}</div>
      </div>
      {pl && (
        <div className="mt-1.5 flex h-7 items-center gap-2 rounded bg-black/30 px-3">
          <span className="truncate font-title text-[16px] font-bold text-cm-mint">{pl.positionLong || '-'}</span>
          <span className="ml-auto flex shrink-0 gap-1">
            {pl.positions.map((t) => (
              <span
                key={t.code}
                title={t.natural ? 'Posição de origem' : undefined}
                className={`rounded px-1.5 font-mono text-[11px] font-bold tracking-wider ${t.natural ? 'bg-cm-dlgborder text-black' : 'bg-cm-dlgdeep text-cm-mint'}`}
              >
                {t.code}
              </span>
            ))}
          </span>
          <span className="shrink-0 font-mono text-[11px] uppercase tracking-wider text-cm-dim">
            Pé preferido: <b className="text-cm-yellow">{pl.preferredFoot ?? '-'}</b>
          </span>
        </div>
      )}
      <div className="mt-1.5">{tabs}</div>
      {squadMenu && <CascadeMenu x={squadMenu.x} y={squadMenu.y} items={squadMenu.items} onClose={() => setSquadMenu(null)} />}
    </div>
  )
}
