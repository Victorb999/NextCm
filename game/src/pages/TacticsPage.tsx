import { useEffect, useState } from 'react'
import { api } from '../api'
import type { ClubProfile } from '../bindings/ClubProfile'
import type { ClubTactics } from '../bindings/ClubTactics'
import type { TacticPlayer } from '../bindings/TacticPlayer'
import { BottomBar, Link, PurpleMenu, Tabs, TitleBand } from '../components/Screen'
import { useEscBack, useNav } from '../nav'

/** "Sobrenome, Nome" -> "Sobrenome" (como os nomes no campo do CM). */
const surname = (listName: string) => listName.split(', ')[0]

/** Nome na lista: "Nome Sobrenome", como na tela de tácticas do CM. */
const fullName = (listName: string) => {
  const [last, first] = listName.split(', ')
  return first ? `${first} ${last}` : last
}

function Row({ p, role, bench, selected, onSelect }: { p: TacticPlayer; role?: string; bench?: boolean; selected: boolean; onSelect: () => void }) {
  const nav = useNav()
  return (
    <div
      onMouseDown={onSelect}
      onDoubleClick={() => nav.go({ kind: 'person', id: p.id })}
      className={`flex h-[19px] items-center gap-2 px-1 text-[13.5px] ${selected ? 'bg-cm-sel' : ''}`}
    >
      <span
        className={`w-7 shrink-0 text-center tabular-nums ${bench ? 'bg-[#1d2d8a] text-white' : 'text-white'}`}
      >
        {p.squadNumber ?? ''}
      </span>
      <span className={`flex-1 truncate ${bench ? 'text-[#8a5ad8]' : 'text-white'}`}>{fullName(p.listName)}</span>
      <span className={`w-24 shrink-0 truncate ${bench ? 'text-[#8a5ad8]' : 'text-white'}`}>{p.position}</span>
      <span className={`w-10 shrink-0 text-right ${bench ? 'text-[#8a5ad8]' : 'text-cm-yellow'}`} title={role ? `Na função ${role}` : undefined}>
        100%
      </span>
    </div>
  )
}

/** Campo com a formação, como a "Vista Geral" do CM. */
function Pitch({ t }: { t: ClubTactics }) {
  const nav = useNav()
  return (
    <div className="relative min-h-0 flex-1 overflow-hidden border border-[#1d5a1d] bg-[repeating-linear-gradient(0deg,#1f6b22_0_36px,#1c6420_36px_72px)]">
      {/* Linhas do campo */}
      <div className="pointer-events-none absolute inset-2 border border-white/40" />
      <div className="pointer-events-none absolute left-2 right-2 top-1/2 border-t border-white/40" />
      <div className="pointer-events-none absolute left-1/2 top-1/2 h-24 w-24 -translate-x-1/2 -translate-y-1/2 rounded-full border border-white/40" />
      <div className="pointer-events-none absolute left-1/2 top-2 h-[16%] w-[44%] -translate-x-1/2 border border-t-0 border-white/40" />
      <div className="pointer-events-none absolute bottom-2 left-1/2 h-[16%] w-[44%] -translate-x-1/2 border border-b-0 border-white/40" />
      {t.starters.map((s, i) => (
        <div
          key={i}
          className="absolute flex -translate-x-1/2 -translate-y-1/2 flex-col items-center"
          style={{ left: `${s.x}%`, top: `${s.y}%` }}
          title={s.role}
        >
          <span
            className={`flex h-6 w-6 items-center justify-center rounded-full border border-black text-[12px] font-bold ${
              s.role === 'GR' ? 'bg-[#0f9a2a] text-white' : 'bg-white text-black'
            }`}
          >
            {s.player?.squadNumber ?? ''}
          </span>
          {s.player && (
            <Link
              onClick={() => nav.go({ kind: 'person', id: s.player!.id })}
              className="whitespace-nowrap text-[13px] font-bold text-cm-yellow [text-shadow:1px_1px_0_#000]"
            >
              {surname(s.player.listName)}
            </Link>
          )}
        </div>
      ))}
    </div>
  )
}

/** Tela "Tácticas <clube>": formação preferida do treinador e o onze escolhido pelo core. */
export default function TacticsPage({ id }: { id: number }) {
  const nav = useNav()
  useEscBack()
  const [t, setT] = useState<ClubTactics | null>(null)
  const [club, setClub] = useState<ClubProfile | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [selected, setSelected] = useState<number | null>(null)

  useEffect(() => {
    api.getClubTactics(id).then(setT, (e) => setError(String(e)))
    api.getClub(id).then(setClub, () => {})
  }, [id])

  const colours = club?.club.fg && club?.club.bg ? { fg: club.club.fg, bg: club.club.bg } : { fg: '#ffffff', bg: '#3a3a3a' }
  const name = club?.club.shortName ?? t?.clubName ?? ''

  return (
    <div className="flex h-full flex-col">
      <TitleBand title={name && `Tácticas ${name}`} fg={colours.fg} bg={colours.bg} />
      <div className="flex shrink-0 items-center gap-1 px-1.5">
        <PurpleMenu label="Tácticas" options={[]} disabled />
        <PurpleMenu label="Vêr" options={[]} disabled />
        <div className="flex-1" />
        <button className="cm-purple h-6 w-40" disabled title="Disponível na Fase 3">
          Último Jogo
        </button>
        <PurpleMenu label="Editar" options={[]} disabled />
      </div>
      {error && <div className="mx-1.5 mt-1.5 bg-red-900 px-2 py-1 text-white">{error}</div>}
      {t && (
        <div className="mx-1.5 mt-1.5 flex min-h-0 flex-1 gap-1.5">
          <div className="flex min-h-0 w-[48%] flex-col">
            <div className="flex h-10 shrink-0 items-center justify-center gap-2 bg-black/70 font-title text-[20px] font-bold text-cm-yellow">
              {t.formation}
              {t.coach && (
                <span className="text-[14px] font-normal text-white">
                  (
                  <Link onClick={() => nav.go({ kind: 'person', id: t.coachId! })} className="text-cm-magenta">
                    {t.coach}
                  </Link>
                  )
                </span>
              )}
            </div>
            <div className="min-h-0 flex-1 overflow-auto bg-black/50 py-1">
              {t.starters.map(
                (s, i) =>
                  s.player && (
                    <Row key={`s${i}`} p={s.player} role={s.role} selected={selected === i} onSelect={() => setSelected(i)} />
                  ),
              )}
              {t.bench.map((p, i) => (
                <Row key={`b${i}`} p={p} bench selected={selected === 100 + i} onSelect={() => setSelected(100 + i)} />
              ))}
            </div>
          </div>
          <div className="flex min-h-0 flex-1 flex-col">
            <Tabs
              tabs={[
                { key: 'overview', label: 'Vista Geral' },
                { key: 'with', label: 'Com Bola', disabled: true },
                { key: 'without', label: 'Sem Bola', disabled: true },
              ]}
              active="overview"
              onChange={() => {}}
            />
            <div className="mt-1 flex min-h-0 flex-1 flex-col">
              <Pitch t={t} />
            </div>
          </div>
        </div>
      )}
      {!t && <div className="flex-1" />}
      <BottomBar left="Cancelar" right="Ok" onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canBack ? nav.back : undefined} />
    </div>
  )
}
