import { useEffect, useMemo, useState } from 'react'
import type { SquadAttributes } from '../bindings/SquadAttributes'
import { api } from '../api'
import type { SquadPlayer } from '../bindings/SquadPlayer'
import type { TrainingProgram } from '../bindings/TrainingProgram'
import type { TrainingView } from '../bindings/TrainingView'
import { Link, BottomBar, Empty, ErrorBox, PurpleMenu, ScreenHeader, Tabs, type MenuOption } from '../components/Screen'
import { InfBadge } from '../components/SquadTable'
import { CoachesDialog, ProgramDialog } from '../components/TrainingDialogs'
import { money } from '../format'
import { useGame } from '../game'
import { useNav } from '../nav'
import { ATTRIBUTE_GROUPS, FILTER_OPTIONS, NO_FILTER, nameClass, passes, squadName, type AttributeGroup, type Filter } from '../squad'

// "<Clube> - Treinos", como no CM: uma aba com todos e uma por programa (com quantos
// jogadores tem); Programa (ou o programa na linha do jogador) muda o programa do jogador,
// Editar abre o diálogo do programa e Preparadores escolhe o que cada treinador treina. Como no
// CM, arrastar um jogador para a aba de um programa muda-o para esse programa. Cancelar / Ok, como nas Tácticas: o core valida e
// grava tudo no Ok.

/** Abas por linha (as restantes vão para a de baixo, como no CLZ). */
const TABS_PER_ROW = 5

type Dialog = { kind: 'program'; program: TrainingProgram } | { kind: 'coaches' } | null

const blank = (id: number): TrainingProgram => ({
  id,
  name: 'Novo Programa',
  fitness: 2,
  tactics: 2,
  shooting: 2,
  technique: 2,
  goalkeeping: 0,
  newPosition: null,
  newSide: null,
})

export default function TrainingPage() {
  const nav = useNav()
  const game = useGame()
  const [view, setView] = useState<TrainingView | null>(null)
  const [programs, setPrograms] = useState<TrainingProgram[]>([])
  const [assign, setAssign] = useState<Map<number, number>>(new Map())
  const [coachAreas, setCoachAreas] = useState<Map<number, boolean[]>>(new Map())
  const [tab, setTab] = useState('all')
  const [filter, setFilter] = useState<Filter>(NO_FILTER)
  const [selected, setSelected] = useState(0)
  const [dialog, setDialog] = useState<Dialog>(null)
  const [error, setError] = useState<string | null>(null)
  // Programas novos ainda por gravar têm id 0, -1, -2...
  const [nextId, setNextId] = useState(0)

  const load = (v: TrainingView) => {
    setView(v)
    setPrograms(v.programs)
    setAssign(new Map(v.assign.map((a) => [a.person, a.program])))
    setCoachAreas(new Map(v.coaches.map((c) => [c.id, c.areas])))
  }
  useEffect(() => {
    api.trainingView().then(load, (e) => setError(String(e)))
  }, [])

  const count = (id: number) => [...assign.values()].filter((p) => p === id).length
  const programName = (id: number | undefined) => programs.find((p) => p.id === id)?.name ?? '-'

  const rows: SquadPlayer[] = useMemo(() => {
    if (!view) return []
    return view.players
      .filter((p) => (tab === 'all' || String(assign.get(p.id)) === tab) && passes(p, filter))
      .sort((a, b) => a.positionOrder - b.positionOrder || a.listName.localeCompare(b.listName))
  }, [view, tab, assign, filter])
  const current = rows[selected]

  // Largar um jogador (arrastado da lista) numa aba de programa
  const dropOnTab = (key: string, data: string) => {
    const player = Number(data)
    if (key === 'all' || !Number.isFinite(player) || !programs.some((p) => String(p.id) === key)) return
    setAssign((m) => new Map(m).set(player, Number(key)))
  }
  const tabs = [{ key: 'all', label: 'Programa' }, ...programs.map((p) => ({ key: String(p.id), label: `${p.name} (${count(p.id)})` }))]

  const giveProgram = (key: string, player = current) => {
    if (key === 'new') {
      setDialog({ kind: 'program', program: blank(nextId) })
      return
    }
    if (!player) return
    setAssign((m) => new Map(m).set(player.id, Number(key)))
  }
  const programMenu: MenuOption<string>[] = [
    ...programs.map((p) => ({ key: String(p.id), label: p.name })),
    'separator',
    { key: 'new', label: 'Novo Programa...' },
  ]
  // O programa da aba aberta, ou o do jogador escolhido na aba Programa
  const editing = programs.find((p) => (tab === 'all' ? p.id === assign.get(current?.id ?? -1) : String(p.id) === tab))

  const saveProgram = (p: TrainingProgram) => {
    setPrograms((list) => (list.some((x) => x.id === p.id) ? list.map((x) => (x.id === p.id ? p : x)) : [...list, p]))
    if (p.id === nextId) setNextId((n) => n - 1)
    setDialog(null)
  }
  const deleteProgram = (id: number) => {
    setPrograms((list) => list.filter((p) => p.id !== id))
    // Quem estava nele volta ao programa do patch (o core decide ao gravar)
    setAssign((m) => new Map([...m].filter(([, p]) => p !== id)))
    if (tab === String(id)) setTab('all')
    setDialog(null)
  }

  const ok = async () => {
    try {
      const players = [...assign].map(([person, program]) => ({ person, program }))
      const coaches = [...coachAreas].map(([person, areas]) => ({ person, areas }))
      await api.setTraining({ programs, players, coaches })
      nav.back()
    } catch (e) {
      setError(String(e))
    }
  }

  // Teclado: setas mudam o jogador, Esc cancela
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (dialog) return
      if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
        e.preventDefault()
        setSelected((s) => Math.max(0, Math.min(rows.length - 1, s + (e.key === 'ArrowUp' ? -1 : 1))))
      } else if (e.key === 'Escape') {
        nav.back()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  })

  const [columns, setColumns] = useState<'general' | 'attributes'>('general')
  const [group, setGroup] = useState<AttributeGroup>('physical')
  const [attributes, setAttributes] = useState<Map<number, SquadAttributes['attributes']>>(new Map())
  const clubId = view?.club.id
  useEffect(() => {
    if (columns !== 'attributes' || clubId === undefined) return
    api.squadAttributes(clubId).then(
      (rows) => setAttributes(new Map(rows.map((r) => [r.person, r.attributes]))),
      () => setAttributes(new Map()),
    )
  }, [columns, clubId])
  const head = 'font-mono text-[11px] font-bold uppercase tracking-[0.1em] text-cm-dim'
  // Vêr Atributos: os atributos de cada jogador do grupo escolhido (do core, como no plantel)
  const groupCols = (ATTRIBUTE_GROUPS.find((g) => g.key === group) ?? ATTRIBUTE_GROUPS[0]).columns
  const cell = (i: number) => `shrink-0 truncate font-mono text-[12.5px] ${i === selected ? 'text-white' : 'text-cm-yellow'}`
  return (
    <div className="relative flex h-full flex-col">
      <ScreenHeader club={view?.club} eyebrow={<span className="text-cm-orange">Treino</span>} title={view ? `${view.club.name} - Treinos` : ''} />
      <Tabs tabs={tabs.slice(0, TABS_PER_ROW)} active={tab} onChange={(k) => (setTab(k), setSelected(0))} onDrop={dropOnTab} />
      {tabs.length > TABS_PER_ROW && (
        <div className="mt-1">
          <Tabs tabs={tabs.slice(TABS_PER_ROW)} active={tab} onChange={(k) => (setTab(k), setSelected(0))} onDrop={dropOnTab} />
        </div>
      )}
      <div className="mx-2 mt-2 flex shrink-0 items-center gap-1.5 rounded-md bg-cm-headdeep px-2 py-1.5">
        <div title={current ? `Mudar o programa de ${squadName(current)}` : undefined}>
          <PurpleMenu
            label="Programa"
            options={programMenu}
            value={String(assign.get(current?.id ?? -1) ?? '')}
            onChange={(k) => giveProgram(k)}
            disabled={!current}
          />
        </div>
        <button
          className="cm-purple h-6 w-40"
          disabled={!editing}
          onClick={() => editing && setDialog({ kind: 'program', program: editing })}
        >
          Editar Programa
        </button>
        <PurpleMenu
          label="Vêr"
          options={[
            { key: 'general', label: 'Geral' },
            { key: 'attributes', label: 'Atributos' },
          ]}
          value={columns}
          onChange={setColumns}
        />
        {columns === 'attributes' && <PurpleMenu label="Atributos" options={ATTRIBUTE_GROUPS} value={group} onChange={setGroup} />}
        <button className="cm-purple h-6 w-40" disabled={!view} onClick={() => setDialog({ kind: 'coaches' })}>
          Preparadores
        </button>
        <div className="flex-1" />
        <PurpleMenu
          label={filter.line || filter.side || filter.avail ? 'Filtrar ✓' : 'Filtrar'}
          options={FILTER_OPTIONS}
          checked={(key) => {
            const [group, value] = key.split(':')
            return group === 'line' ? filter.line === value : group === 'side' ? filter.side === value : filter.avail === value
          }}
          onChange={(key) => {
            const [group, value] = key.split(':')
            setFilter((f) =>
              group === 'line'
                ? { ...f, line: value }
                : group === 'side'
                  ? { ...f, side: value }
                  : { ...f, avail: f.avail === value ? '' : (value as Filter['avail']) },
            )
            setSelected(0)
          }}
          align="right"
        />
      </div>

      <div className="mx-2 mt-2 flex min-h-0 flex-1 flex-col">
        <ErrorBox className="mb-2">{error}</ErrorBox>
        <div className="min-h-0 flex-1 overflow-auto">
          {rows.length ? (
            <div className="rounded-md bg-cm-surface">
              <div className={`flex h-7 items-center gap-2 border-b border-cm-line bg-black/30 px-2 ${head}`}>
                <span className="w-24">Programa</span>
                <span className="w-10 text-center">Inf</span>
                <span className="min-w-28 flex-1">Nome</span>
                <span className="w-24">Posição</span>
                <span className="w-12 text-right">Idade</span>
                {columns === 'general' ? (
                  <>
                    <span className="w-20">Forma</span>
                    <span className="w-20">Moral</span>
                    <span className="w-14 text-right">Cond.</span>
                    <span className="w-20 text-right">Valor</span>
                  </>
                ) : (
                  groupCols.map(([key, l]) => (
                    <span key={key} className="w-11 text-right">
                      {l}
                    </span>
                  ))
                )}
              </div>
              <div className="cm-zebra">
                {rows.map((p, i) => (
                  <div
                    key={p.id}
                    onClick={() => setSelected(i)}
                    draggable
                    onDragStart={(e) => {
                      e.dataTransfer.setData('text/plain', String(p.id))
                      e.dataTransfer.effectAllowed = 'move'
                      setSelected(i)
                    }}
                    title="Arraste para a aba de um programa para o mudar"
                    className={`flex h-6 cursor-grab items-center gap-2 px-2 ${i === selected ? '!bg-cm-sel' : ''}`}
                  >
                    {/* O programa na linha: um clique abre a lista para o mudar */}
                    <span className="w-24 shrink-0" onClick={() => setSelected(i)}>
                      <PurpleMenu
                        label="Programa"
                        options={programMenu}
                        value={String(assign.get(p.id) ?? '')}
                        onChange={(k) => giveProgram(k, p)}
                        buttonClass={`flex w-24 items-center gap-0.5 !font-sans font-semibold hover:text-white ${cell(i)}`}
                        trigger={
                          <>
                            <span className="min-w-0 flex-1 truncate text-left">{programName(assign.get(p.id))}</span>
                            <span className="text-[10px] text-cm-purplelight">▼</span>
                          </>
                        }
                      />
                    </span>
                    <span className="w-10 shrink-0 text-center font-mono text-[12px]">
                      <InfBadge info={p.info} />
                    </span>
                    <span className={`min-w-28 flex-1 truncate text-[15px] font-semibold ${nameClass(p)}`}>
                      <Link onClick={() => nav.go({ kind: 'person', id: p.id })}>{squadName(p)}</Link>
                    </span>
                    <span className={`w-24 ${cell(i)}`}>{p.position}</span>
                    <span className={`w-12 text-right ${cell(i)}`}>{p.age ?? '-'}</span>
                    {columns === 'general' ? (
                      <>
                        <span className={`w-20 ${cell(i)}`}>{p.form.length ? p.form.map((f) => Math.round(f)).join(' ') : '-'}</span>
                        <span className={`w-20 ${cell(i)}`}>{p.morale ?? '-'}</span>
                        <span className={`w-14 text-right ${cell(i)}`}>{p.condition === null ? '-' : `${p.condition}%`}</span>
                        <span className={`w-20 text-right ${cell(i)}`}>{p.free ? 'Livre' : money(p.value)}</span>
                      </>
                    ) : (
                      groupCols.map(([key]) => (
                        <span key={key} className={`w-11 text-right ${cell(i)}`}>
                          {attributes.get(p.id)?.[key] ?? '-'}
                        </span>
                      ))
                    )}
                  </div>
                ))}
              </div>
            </div>
          ) : (
            <Empty>{view ? 'Nenhum jogador neste programa.' : 'A carregar...'}</Empty>
          )}
        </div>
      </div>

      <BottomBar left="Cancelar" right="Ok" onLeft={nav.back} onRight={game.mode === 'game' && view ? () => void ok() : undefined} />
      {dialog?.kind === 'program' && (
        <ProgramDialog
          initial={dialog.program}
          canDelete={programs.length > 1 && programs.some((p) => p.id === dialog.program.id)}
          onCancel={() => setDialog(null)}
          onOk={saveProgram}
          onDelete={() => deleteProgram(dialog.program.id)}
        />
      )}
      {dialog?.kind === 'coaches' && view && (
        <CoachesDialog
          coaches={view.coaches}
          areas={coachAreas}
          onCancel={() => setDialog(null)}
          onOk={(a) => {
            setCoachAreas(a)
            setDialog(null)
          }}
        />
      )}
    </div>
  )
}
