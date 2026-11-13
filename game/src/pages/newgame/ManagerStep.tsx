import { useMemo } from 'react'
import type { ReactNode } from 'react'
import type { NationOption } from '../../bindings/NationOption'
import type { NationProfile } from '../../bindings/NationProfile'
import PickList, { type PickGroup } from '../../components/PickList'
import { EXPERIENCES, LICENSES } from '../../format'
import ClubCard from './ClubCard'

// Passo 2 do Novo Jogo, como o "New User" do FM08: os dados do treinador em cima e, em baixo, os
// clubes de uma nação escolhida à esquerda e a ficha do clube à direita.

export type ManagerForm = {
  firstName: string
  lastName: string
  birth: string
  nationId: number | null
  license: number
  experience: number
  favouriteClub: number | null
}

const FIELD = 'cm-field h-7 min-w-0 flex-1 text-[15px]'

/** Máscara dd.mm.aaaa: só dígitos, com os pontos postos sozinhos. */
function maskDate(text: string): string {
  const d = text.replace(/\D/g, '').slice(0, 8)
  return [d.slice(0, 2), d.slice(2, 4), d.slice(4)].filter(Boolean).join('.')
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="flex min-w-0 items-center gap-3">
      <span className="w-36 shrink-0 text-[14px] text-cm-dlgtext">{label}</span>
      {children}
    </label>
  )
}

export default function ManagerStep({
  form,
  onForm,
  nations,
  playable,
  profiles,
  clubNation,
  onClubNation,
  club,
  onClub,
  onStart,
}: {
  form: ManagerForm
  onForm: (patch: Partial<ManagerForm>) => void
  nations: NationOption[]
  /** As nações escolhidas no passo 1 (só nelas se pode treinar). */
  playable: number[]
  /** Os clubes de cada nação escolhida. */
  profiles: Map<number, NationProfile>
  clubNation: number | null
  onClubNation: (id: number) => void
  club: number | null
  onClub: (id: number) => void
  onStart: (id: number) => void
}) {
  const nation = clubNation === null ? undefined : profiles.get(clubNation)

  // Clubes por divisão, como na tela da nação
  const groups: PickGroup[] = useMemo(() => {
    const map = new Map<string, PickGroup>()
    for (const c of nation?.clubs ?? []) {
      const key = c.division ?? 'Sem divisão'
      if (!map.has(key)) map.set(key, { title: key, items: [] })
      map.get(key)!.items.push({ id: c.club.id, label: c.club.shortName })
    }
    return [...map.values()]
  }, [nation])

  const random = () => {
    const all = nation?.clubs.filter((c) => c.division) ?? []
    const pick = all[Math.floor(Math.random() * all.length)]
    if (pick) onClub(pick.club.id)
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <div className="shrink-0 rounded bg-black/40 px-3 pb-3 pt-2">
        <div className="pb-2 font-mono text-[12px] font-bold uppercase tracking-[0.12em] text-cm-orange">Treinador</div>
        <div className="grid grid-cols-3 gap-x-6 gap-y-2">
          <Field label="Nome">
            <input autoFocus className={FIELD} value={form.firstName} onChange={(e) => onForm({ firstName: e.target.value })} />
          </Field>
          <Field label="Nacionalidade">
            <select className={FIELD} value={form.nationId ?? ''} onChange={(e) => onForm({ nationId: Number(e.target.value) })}>
              {nations.map((n) => (
                <option key={n.id} value={n.id}>
                  {n.name}
                </option>
              ))}
            </select>
          </Field>
          <Field label="Experiência Passada">
            <select className={FIELD} value={form.experience} onChange={(e) => onForm({ experience: Number(e.target.value) })}>
              {EXPERIENCES.map((l, i) => (
                <option key={l} value={i}>
                  {l}
                </option>
              ))}
            </select>
          </Field>
          <Field label="Sobrenome">
            <input className={FIELD} value={form.lastName} onChange={(e) => onForm({ lastName: e.target.value })} />
          </Field>
          <Field label="Qualificações">
            <select className={FIELD} value={form.license} onChange={(e) => onForm({ license: Number(e.target.value) })}>
              {LICENSES.map((l, i) => (
                <option key={l} value={i}>
                  {l}
                </option>
              ))}
            </select>
          </Field>
          <Field label="Clube do Coração">
            <select
              className={FIELD}
              value={form.favouriteClub ?? ''}
              onChange={(e) => onForm({ favouriteClub: e.target.value ? Number(e.target.value) : null })}
            >
              <option value="">Nenhum</option>
              {playable.map((id) => {
                const p = profiles.get(id)
                if (!p) return null
                return (
                  <optgroup key={id} label={p.name}>
                    {[...p.clubs]
                      .filter((c) => c.division)
                      .sort((a, b) => a.club.shortName.localeCompare(b.club.shortName))
                      .map((c) => (
                        <option key={c.club.id} value={c.club.id}>
                          {c.club.shortName}
                        </option>
                      ))}
                  </optgroup>
                )
              })}
            </select>
          </Field>
          <Field label="Data de Nascimento">
            <input
              className={FIELD}
              placeholder="dd.mm.aaaa"
              inputMode="numeric"
              maxLength={10}
              value={form.birth}
              onChange={(e) => onForm({ birth: maskDate(e.target.value) })}
            />
          </Field>
        </div>
      </div>

      <div className="flex min-h-0 flex-1 gap-3">
        <div className="flex min-h-0 w-[46%] flex-col rounded bg-black/40">
          <div className="flex shrink-0 items-center gap-2 px-3 pb-1 pt-2">
            <span className="font-mono text-[12px] font-bold uppercase tracking-[0.12em] text-cm-orange">Escolher Clube</span>
            <select
              className="cm-field ml-auto h-7 w-44 text-[15px]"
              value={clubNation ?? ''}
              onChange={(e) => onClubNation(Number(e.target.value))}
            >
              {playable.map((id) => (
                <option key={id} value={id}>
                  {nations.find((n) => n.id === id)?.name ?? id}
                </option>
              ))}
            </select>
            <button className="cm-purple h-7 w-28" onClick={random}>
              Aleatório
            </button>
          </div>
          {nation && (
            <PickList
              key={nation.id}
              groups={groups}
              selectedId={club ?? undefined}
              onSelect={(c) => onClub(c.id)}
              onOpen={(c) => onStart(c.id)}
              empty="Nenhum clube com liga nesta nação."
            />
          )}
        </div>
        <div className="flex min-h-0 flex-1 flex-col overflow-auto rounded bg-black/40 p-2">
          <ClubCard id={club} />
        </div>
      </div>
    </div>
  )
}
