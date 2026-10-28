import { useEffect, useMemo, useState } from 'react'
import type { LeagueChoice } from '../bindings/LeagueChoice'
import type { LeagueNation } from '../bindings/LeagueNation'
import type { StartSeason } from '../bindings/StartSeason'
import LeaguesStep from './LeaguesStep'
import StartStep from './StartStep'
import { api } from '../api'
import type { NationOption } from '../bindings/NationOption'
import type { NationProfile } from '../bindings/NationProfile'
import { DialogFrame, DialogRow, Overlay } from '../components/Dialog'
import Loading from '../components/Loading'
import PickList, { type PickGroup, type PickItem } from '../components/PickList'
import { useGame } from '../game'
import { useNav } from '../nav'

/** "13.7.1990" -> "1990-07-13"; `null` se vazio, `undefined` se inválido. */
function parseBirth(text: string): string | null | undefined {
  const t = text.trim()
  if (!t) return null
  const m = t.match(/^(\d{1,2})[./-](\d{1,2})[./-](\d{4})$/)
  if (!m) return undefined
  const [d, mo, y] = [Number(m[1]), Number(m[2]), Number(m[3])]
  const date = new Date(y, mo - 1, d)
  if (date.getDate() !== d || date.getMonth() !== mo - 1 || y < 1920 || y > 2010) return undefined
  return `${y}-${String(mo).padStart(2, '0')}-${String(d).padStart(2, '0')}`
}

const FIELD = 'cm-field h-7 w-72 text-[15px]'

/** Máscara dd.mm.aaaa: só dígitos, com os pontos postos sozinhos. */
function maskDate(text: string): string {
  const d = text.replace(/\D/g, '').slice(0, 8)
  return [d.slice(0, 2), d.slice(2, 4), d.slice(4)].filter(Boolean).join('.')
}

/** "Novo Jogo": as ligas, a época, os dados do treinador e o clube. */
export default function NewGamePage() {
  const game = useGame()
  const nav = useNav()
  const [step, setStep] = useState<'leagues' | 'start' | 'manager' | 'club'>('leagues')
  const [season, setSeason] = useState<StartSeason>('europe')
  // "Escolher Campeonato(s)": por omissão o Brasil e a Inglaterra escolhidos, o resto de fundo
  const [leagueNations, setLeagueNations] = useState<LeagueNation[]>([])
  const [choices, setChoices] = useState<LeagueChoice[]>([])
  const [nations, setNations] = useState<NationOption[]>([])
  const [form, setForm] = useState({ saveName: 'Jogo 1', firstName: '', lastName: '', birth: '', nationId: null as number | null })
  const [clubNation, setClubNation] = useState<number | null>(null)
  const [nation, setNation] = useState<NationProfile | null>(null)
  const [club, setClub] = useState<PickItem | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    api.listNations().then((list) => {
      setNations(list)
      const brazil = list.find((n) => n.name === 'Brazil')?.id ?? null
      setForm((f) => ({ ...f, nationId: f.nationId ?? brazil }))
      setClubNation((c) => c ?? brazil)
    }, (e) => setError(String(e)))
  }, [])

  useEffect(() => {
    if (clubNation !== null) api.getNation(clubNation).then(setNation, (e) => setError(String(e)))
  }, [clubNation])

  useEffect(() => {
    api.leagueNations().then((list) => {
      setLeagueNations(list)
      setChoices(list.map((n) => ({ nationId: n.nationId, mode: ['Brazil', 'England'].includes(n.name) ? 'play' : 'background' })))
    }, (e) => setError(String(e)))
  }, [])
  const playable = choices.filter((c) => c.mode === 'play').map((c) => c.nationId)
  const calendarPlay = leagueNations.some((n) => n.calendar && playable.includes(n.nationId))

  const changeClubNation = (id: number) => {
    setNation(null)
    setClub(null)
    setClubNation(id)
  }

  // Clubes por divisão, como na tela da nação
  const groups: PickGroup[] = useMemo(() => {
    const map = new Map<string, PickGroup>()
    for (const c of nation?.clubs ?? []) {
      const key = c.division ?? 'Sem divisão'
      if (!map.has(key)) map.set(key, { title: key, items: [] })
      map.get(key)!.items.push({ id: c.club.id, label: c.club.shortName, detail: c.reputation ?? '' })
    }
    return [...map.values()]
  }, [nation])

  const set = (patch: Partial<typeof form>) => setForm((f) => ({ ...f, ...patch }))

  const next = () => {
    if (!form.firstName.trim() || !form.lastName.trim()) return setError('Indique o nome e o apelido do treinador.')
    if (!form.saveName.trim()) return setError('Escolha um nome para o jogo.')
    if (parseBirth(form.birth) === undefined) return setError('Data de nascimento inválida (use dd.mm.aaaa).')
    setError(null)
    setStep('club')
  }

  const start = async (chosen: PickItem | null) => {
    if (!chosen || busy) return
    setBusy(true)
    setError(null)
    try {
      const info = await api.newGame({
        saveName: form.saveName.trim(),
        firstName: form.firstName.trim(),
        lastName: form.lastName.trim(),
        birthDate: parseBirth(form.birth) ?? null,
        nationId: form.nationId,
        clubId: chosen.id,
        leagues: choices,
        start: season,
      })
      game.update(info)
      nav.go({ kind: 'news' })
    } catch (e) {
      setError(String(e))
      setBusy(false)
    }
  }

  const nationSelect = (value: number | null, onChange: (id: number) => void, only?: number[]) => (
    <select className={FIELD} value={value ?? ''} onChange={(e) => onChange(Number(e.target.value))}>
      {nations.filter((n) => !only || only.includes(n.id)).map((n) => (
        <option key={n.id} value={n.id}>
          {n.name}
        </option>
      ))}
    </select>
  )

  if (step === 'leagues') {
    return (
      <LeaguesStep
        nations={leagueNations}
        choices={choices}
        onChange={setChoices}
        onCancel={() => game.setMode('start')}
        error={error}
        onNext={() => {
          if (playable.length === 0) return setError('Escolha pelo menos uma liga para jogar.')
          setError(null)
          // O clube escolhe-se numa nação escolhida
          if (clubNation === null || !playable.includes(clubNation)) changeClubNation(playable[0])
          if (!calendarPlay) setSeason('europe')
          setStep('start')
        }}
      />
    )
  }

  if (step === 'start') {
    return (
      <StartStep
        value={season}
        onChange={setSeason}
        brazil={calendarPlay}
        onCancel={() => setStep('leagues')}
        onNext={() => setStep('manager')}
      />
    )
  }

  if (step === 'manager') {
    return (
      <Overlay>
        <DialogFrame title="Novo Jogo — Treinador" onCancel={() => setStep('start')} cancelLabel="Atrás" onOk={next} okLabel="Seguinte">
          <form
            className="space-y-2 py-1"
            onSubmit={(e) => {
              e.preventDefault()
              next()
            }}
          >
            <DialogRow label="Nome do Jogo">
              <input className={FIELD} value={form.saveName} onChange={(e) => set({ saveName: e.target.value })} />
            </DialogRow>
            <DialogRow label="Nome">
              <input autoFocus className={FIELD} value={form.firstName} onChange={(e) => set({ firstName: e.target.value })} />
            </DialogRow>
            <DialogRow label="Apelido">
              <input className={FIELD} value={form.lastName} onChange={(e) => set({ lastName: e.target.value })} />
            </DialogRow>
            <DialogRow label="Data de Nascimento">
              <input
                className={FIELD}
                placeholder="dd.mm.aaaa"
                inputMode="numeric"
                maxLength={10}
                value={form.birth}
                onChange={(e) => set({ birth: maskDate(e.target.value) })}
              />
            </DialogRow>
            <DialogRow label="Nacionalidade">{nationSelect(form.nationId, (id) => set({ nationId: id }))}</DialogRow>
            {error && <div className="bg-red-900 px-2 py-1 text-white">{error}</div>}
            <button type="submit" className="hidden" />
          </form>
        </DialogFrame>
      </Overlay>
    )
  }

  return (
    <Overlay>
      <DialogFrame
        title="Novo Jogo — Escolher Clube"
        width={760}
        header={
          <div className="flex shrink-0 items-center gap-3 px-1 pt-2 text-white">
            <span className="font-title text-[17px]">Nação</span>
            {nationSelect(clubNation, changeClubNation, playable)}
            <span className="flex-1 text-right font-title text-[17px] text-cm-yellow">
              {club ? `Clube: ${club.label}` : 'Escolha um clube'}
            </span>
          </div>
        }
        onCancel={() => setStep('manager')}
        cancelLabel="Atrás"
        onOk={club ? () => void start(club) : undefined}
        okLabel={busy ? 'A criar o jogo…' : 'Começar'}
      >
        {error && <div className="mb-2 bg-red-900 px-2 py-1 text-white">{error}</div>}
        <div className="flex h-[52vh] flex-col bg-black/40">
          {nation && <PickList key={nation.id} groups={groups} onSelect={setClub} onOpen={(c) => void start(c)} empty="Nenhum clube com liga nesta nação." />}
        </div>
      </DialogFrame>
      {busy && (
        <Loading title="Novo Jogo" detail={season === 'brazil' ? 'A jogar o 2º semestre de 2025…' : 'A criar o jogo e o calendário…'} />
      )}
    </Overlay>
  )
}
