import { useEffect, useState } from 'react'
import { api } from '../api'
import type { ClubListItem } from '../bindings/ClubListItem'
import type { NationOption } from '../bindings/NationOption'
import type { ScoutList } from '../bindings/ScoutList'
import { money } from '../format'
import { useNav } from '../nav'
import { DialogFrame, Overlay } from './Dialog'
import { ErrorBox, Link, PurpleMenu } from './Screen'

// Aba "Olheiro #N" da Pesquisa Jogador & Staff: a tarefa do olheiro ("Designar Olheiro") e os
// jogadores que ele já observou, com a recomendação em estrelas. As regras ficam no core.

type Kind = 'club' | 'opponent' | 'youth' | 'nation' | 'region'
const TASKS: { key: Kind; label: string }[] = [
  { key: 'club', label: 'Clube' },
  { key: 'opponent', label: 'Próximo Adversário' },
  { key: 'youth', label: 'Jogadores Jovens' },
  { key: 'nation', label: 'País' },
  { key: 'region', label: 'Região' },
]
const REGIONS = [
  { id: 5, name: 'América do Sul' },
  { id: 2, name: 'Europa' },
  { id: 3, name: 'América do Norte' },
  { id: 0, name: 'África' },
  { id: 1, name: 'Ásia' },
  { id: 4, name: 'Oceânia' },
]

/** Escolher o alvo de uma tarefa: um clube (pelo nome), um país ou uma região. */
function TargetDialog({ kind, nations, onCancel, onPick }: { kind: Kind; nations: NationOption[]; onCancel: () => void; onPick: (id: number) => void }) {
  const [text, setText] = useState('')
  const [clubs, setClubs] = useState<ClubListItem[]>([])
  useEffect(() => {
    if (kind !== 'club') return
    const t = setTimeout(() => {
      api.searchClubs(text).then(setClubs, () => setClubs([]))
    }, 150)
    return () => clearTimeout(t)
  }, [kind, text])
  const items: { id: number; label: string; detail?: string }[] =
    kind === 'club'
      ? clubs.map((c) => ({ id: c.club.id, label: c.club.name, detail: [c.division, c.nation].filter(Boolean).join(' · ') }))
      : kind === 'nation'
        ? nations.map((n) => ({ id: n.id, label: n.name }))
        : REGIONS.map((r) => ({ id: r.id, label: r.name }))
  const title = kind === 'club' ? 'Observar Clube' : kind === 'nation' ? 'Observar País' : 'Observar Região'
  return (
    <Overlay>
      <DialogFrame title={title} width={520} onCancel={onCancel}>
        {kind === 'club' && (
          <input
            autoFocus
            value={text}
            onChange={(e) => setText(e.target.value)}
            placeholder="Nome do clube"
            className="mb-2 h-7 w-full border border-[#1d3a1f] bg-black px-2 text-[15px] text-cm-yellow outline-none"
          />
        )}
        <div className="max-h-[360px] overflow-auto">
          {items.map((i) => (
            <button key={i.id} onClick={() => onPick(i.id)} className="flex w-full items-center justify-between px-2 py-0.5 text-left hover:bg-cm-sel">
              <span className="font-title text-[17px] text-white">{i.label}</span>
              {i.detail && <span className="text-cm-yellow">{i.detail}</span>}
            </button>
          ))}
        </div>
      </DialogFrame>
    </Overlay>
  )
}

export default function ScoutPanel({ slot, nations }: { slot: number; nations: NationOption[] }) {
  const nav = useNav()
  const [list, setList] = useState<ScoutList | null>(null)
  const [picking, setPicking] = useState<Kind | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    api.scoutList(slot).then(
      (l) => {
        setList(l)
        setError(null)
      },
      (e) => setError(String(e)),
    )
  }, [slot])

  const run = async (p: Promise<ScoutList>) => {
    try {
      setList(await p)
      setError(null)
    } catch (e) {
      setError(String(e))
    }
  }
  const assign = (kind: Kind) => {
    if (kind === 'opponent' || kind === 'youth') void run(api.assignScout(slot, kind, null))
    else setPicking(kind)
  }
  const noScout = !!list && !list.scout

  return (
    <>
      <div className="flex shrink-0 items-center gap-1 px-1.5 pt-2">
        <PurpleMenu label="Designar Olheiro" options={TASKS} disabled={noScout} onChange={assign} />
        <div className="flex-1" />
        <button className="cm-purple h-6 w-40" disabled={noScout || !list?.task} onClick={() => void run(api.clearScout(slot))}>
          Apagar
        </button>
      </div>
      <div className="mx-1.5 mt-1 flex min-h-0 flex-1 flex-col rounded-md bg-[#1a1b21]">
        <div className="shrink-0 px-2 pt-1 text-center text-cm-magenta">
          {noScout ? 'O clube não tem um olheiro para esta aba.' : (list?.task ?? `${list?.scout ?? 'O olheiro'} está sem tarefa: use Designar Olheiro.`)}
        </div>
        <div className="shrink-0 py-1 text-center font-title text-xl font-bold text-cm-yellow">Olheiro #{slot}</div>
        <ErrorBox>{error}</ErrorBox>
        <div className="min-h-0 flex-1 overflow-auto">
          <table className="w-full table-fixed border-collapse">
            <colgroup>
              <col className="w-20" />
              <col className="w-12" />
              <col />
              <col className="w-[18%]" />
              <col className="w-[16%]" />
              <col className="w-[13%]" />
              <col className="w-24" />
            </colgroup>
            <thead className="sticky top-0 z-10">
              <tr className="h-6">
                {['Rec', 'Inf', 'Nome', 'Clube', 'Base', 'Posição', 'Valor'].map((h) => (
                  <th key={h} className={`border-x border-black bg-cm-head px-2 font-normal text-white ${['Nome', 'Clube', 'Base', 'Posição'].includes(h) ? 'text-left' : ''}`}>
                    {h}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {list?.finds.map((f) => (
                <tr key={f.id} className="h-[22px] border-b border-cm-line">
                  <td className="bg-black/40 px-2 text-center text-cm-yellow">{'*'.repeat(f.stars)}</td>
                  <td className="px-1 text-center">{f.injured && <span className="bg-[#b8560c] px-0.5 font-bold text-white">Les</span>}</td>
                  <td className="truncate px-2 text-white">
                    <Link onClick={() => nav.go({ kind: 'person', id: f.id })}>{f.listName}</Link>
                  </td>
                  <td className="truncate px-2 text-cm-magenta">
                    {f.club ? <Link onClick={() => nav.go({ kind: 'club', id: f.club!.id })}>{f.club.shortName}</Link> : 'Sem clube'}
                  </td>
                  <td className="truncate px-2 text-cm-yellow">{f.base ?? '-'}</td>
                  <td className="truncate px-2 text-cm-orange">{f.position}</td>
                  <td className="bg-black/40 px-2 text-right tabular-nums">{f.value === null ? '-' : money(f.value)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <div className="shrink-0 py-1.5 text-center font-title text-2xl font-bold text-cm-magenta">
          {(list?.finds.length ?? 0).toLocaleString('pt-BR')} jogadores encontrados
        </div>
      </div>
      {picking && (
        <TargetDialog
          kind={picking}
          nations={nations}
          onCancel={() => setPicking(null)}
          onPick={(id) => {
            setPicking(null)
            void run(api.assignScout(slot, picking, id))
          }}
        />
      )}
    </>
  )
}
