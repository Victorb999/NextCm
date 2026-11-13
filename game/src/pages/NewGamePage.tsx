import { useEffect, useState } from 'react'
import type { GameInfo } from '../bindings/GameInfo'
import type { LeagueChoice } from '../bindings/LeagueChoice'
import type { LeagueNation } from '../bindings/LeagueNation'
import type { NationOption } from '../bindings/NationOption'
import type { NationProfile } from '../bindings/NationProfile'
import type { StartSeason } from '../bindings/StartSeason'
import { api } from '../api'
import { DialogFrame, Overlay } from '../components/Dialog'
import Loading from '../components/Loading'
import { ErrorBox } from '../components/Screen'
import { useGame } from '../game'
import { useNav, type Screen } from '../nav'
import { selectionOf } from '../selection'
import LeaguesStep, { type GameOptions } from './LeaguesStep'
import ManagerStep, { type ManagerForm } from './newgame/ManagerStep'

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

const STEPS = ['Campeonatos', 'Treinador e Clube'] as const

/**
 * "Novo Jogo" em tela cheia e em 2 passos, como o do FM08: (1) os campeonatos e as opções do
 * jogo; (2) os dados do treinador e o clube, com a ficha de cada um. No fim, o adjunto pergunta
 * o que o treinador quer fazer primeiro.
 */
export default function NewGamePage() {
  const game = useGame()
  const nav = useNav()
  const [step, setStep] = useState(0)
  const [season, setSeason] = useState<StartSeason>('europe')
  const [saveName, setSaveName] = useState('Jogo 1')
  // "Escolher Campeonato(s)": por omissão o Brasil e a Inglaterra escolhidos, o resto de fundo
  const [leagueNations, setLeagueNations] = useState<LeagueNation[]>([])
  const [choices, setChoices] = useState<LeagueChoice[]>([])
  const [options, setOptions] = useState<GameOptions>({ realPlayers: true, masking: true })
  const [nations, setNations] = useState<NationOption[]>([])
  const [form, setForm] = useState<ManagerForm>({
    firstName: '',
    lastName: '',
    birth: '',
    nationId: null,
    license: 0,
    experience: 0,
    favouriteClub: null,
  })
  const [profiles, setProfiles] = useState<Map<number, NationProfile>>(new Map())
  const [clubNation, setClubNation] = useState<number | null>(null)
  const [club, setClub] = useState<number | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  // O jogo criado, à espera da escolha no diálogo do adjunto
  const [created, setCreated] = useState<GameInfo | null>(null)
  // O passo da criação (o core di-lo enquanto o newGame corre)
  const [creation, setCreation] = useState<{ label: string; done: number } | null>(null)
  useEffect(() => {
    if (!busy) return
    const t = setInterval(() => {
      api.creationProgress().then(
        (p) => p.label && setCreation(p),
        () => {},
      )
    }, 300)
    return () => clearInterval(t)
  }, [busy])

  useEffect(() => {
    // O nome por omissão: o primeiro "Jogo N" que ainda não existe
    api.listSaves().then(
      (saves) => {
        const taken = new Set(saves.map((s) => s.name.toLowerCase()))
        let n = 1
        while (taken.has(`jogo ${n}`)) n++
        setSaveName((s) => (s === 'Jogo 1' ? `Jogo ${n}` : s))
      },
      () => {},
    )
    api.listNations().then(
      (list) => {
        setNations(list)
        const brazil = list.find((n) => n.name === 'Brazil')?.id ?? null
        setForm((f) => ({ ...f, nationId: f.nationId ?? brazil }))
      },
      (e) => setError(String(e)),
    )
    api.leagueNations().then(
      (list) => {
        setLeagueNations(list)
        setChoices(list.map((n) => ({ nationId: n.nationId, mode: ['Brazil', 'England'].includes(n.name) ? 'play' : 'background' })))
      },
      (e) => setError(String(e)),
    )
  }, [])

  const playable = choices.filter((c) => c.mode === 'play').map((c) => c.nationId)

  // Os clubes das nações escolhidas (a lista do passo 2 e o Clube do Coração)
  useEffect(() => {
    if (step !== 1) return
    for (const id of playable) {
      if (profiles.has(id)) continue
      api.getNation(id).then(
        (p) => setProfiles((m) => new Map(m).set(id, p)),
        (e) => setError(String(e)),
      )
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [step, playable.join(',')])

  const changeClubNation = (id: number) => {
    setClub(null)
    setClubNation(id)
  }

  const toManager = () => {
    if (playable.length === 0) return setError('Escolha pelo menos um campeonato para jogar.')
    if (!saveName.trim()) return setError('Escolha um nome para o jogo.')
    const calendar = leagueNations.some((n) => n.calendar && playable.includes(n.nationId))
    if (!calendar) setSeason('europe')
    // O clube escolhe-se numa nação escolhida
    if (clubNation === null || !playable.includes(clubNation)) changeClubNation(playable[0])
    setError(null)
    setStep(1)
  }

  const start = async (clubId: number | null) => {
    if (busy) return
    if (!form.firstName.trim() || !form.lastName.trim()) return setError('Indique o nome e o sobrenome do treinador.')
    if (parseBirth(form.birth) === undefined) return setError('Data de nascimento inválida (use dd.mm.aaaa).')
    if (clubId === null) return setError('Escolha um clube.')
    setCreation(null)
    setBusy(true)
    setError(null)
    try {
      const info = await api.newGame({
        saveName: saveName.trim(),
        firstName: form.firstName.trim(),
        lastName: form.lastName.trim(),
        birthDate: parseBirth(form.birth) ?? null,
        nationId: form.nationId,
        clubId,
        leagues: choices,
        start: season,
        license: form.license,
        realPlayers: options.realPlayers,
        masking: options.masking,
        experience: form.experience,
        favouriteClub: form.favouriteClub,
      })
      setCreated(info)
    } catch (e) {
      setError(String(e))
    }
    setBusy(false)
  }

  // O assistente ao começar (FM08): o que o treinador quer fazer primeiro
  const begin = async (choice: 'news' | 'auto' | 'squad' | 'search') => {
    if (!created) return
    try {
      if (choice === 'auto') {
        const t = await api.getClubTactics(created.club.id)
        await api.setTactics({ ...selectionOf(t), auto: true })
      }
    } catch (e) {
      setError(String(e))
    }
    const info = (await api.gameInfo().catch(() => null)) ?? created
    game.update(info)
    const route: Screen =
      choice === 'squad' ? { kind: 'club', id: created.club.id } : choice === 'search' ? { kind: 'search' } : { kind: 'news' }
    nav.go(route)
  }

  if (created) {
    const option = (key: 'news' | 'auto' | 'squad' | 'search', label: string, hint: string) => (
      <button
        key={key}
        onClick={() => void begin(key)}
        className="flex h-14 w-full flex-col items-center justify-center rounded bg-[#01010199] font-title text-[21px] leading-tight hover:bg-[#010101cc]"
      >
        {label}
        <span className="font-sans text-[12px] text-cm-dlgtext">{hint}</span>
      </button>
    )
    return (
      <Overlay>
        <DialogFrame title={`Bem-vindo ao ${created.club.shortName}`} width={520}>
          <div className="pb-2 text-center text-[15px] text-white">O seu adjunto pergunta: o que quer fazer primeiro?</div>
          <div className="space-y-2 pb-1">
            {option('news', 'Ler as Mensagens', 'A Direcção, os media e o relatório do plantel')}
            {option('auto', 'O Adjunto Escolhe a Equipa', 'Ele monta o onze e o banco na táctica do clube')}
            {option('squad', 'Ver o Plantel', 'Os jogadores e os contratos')}
            {option('search', 'Procurar Jogadores', 'A Pesquisa Avançada para o mercado')}
          </div>
          <ErrorBox className="mt-2">{error}</ErrorBox>
        </DialogFrame>
      </Overlay>
    )
  }

  return (
    <Overlay>
      <DialogFrame
        title="Novo Jogo"
        full
        header={
          <div className="flex shrink-0 items-center gap-1 pt-1.5">
            {STEPS.map((label, i) => (
              <div
                key={label}
                className={`flex h-8 flex-1 items-center gap-2 rounded px-3 font-title text-[18px] ${
                  i === step ? 'bg-black/60 text-cm-yellow' : 'bg-black/25 text-cm-dlgtext'
                }`}
              >
                <span className="font-mono text-[13px]">{i + 1}</span>
                {label}
              </div>
            ))}
          </div>
        }
        onCancel={() => (step === 0 ? game.setMode('start') : setStep(0))}
        cancelLabel="Atrás"
        onOk={step === 0 ? toManager : () => void start(club)}
        okLabel={step === 0 ? 'Seguinte' : busy ? 'A criar o jogo…' : 'Começar'}
      >
        <div className="flex h-full min-h-0 flex-col gap-2">
          <ErrorBox className="shrink-0">{error}</ErrorBox>
          <div className="min-h-0 flex-1">
            {step === 0 ? (
              <LeaguesStep
                nations={leagueNations}
                choices={choices}
                onChange={setChoices}
                options={options}
                onOptions={setOptions}
                season={season}
                onSeason={setSeason}
                saveName={saveName}
                onSaveName={setSaveName}
              />
            ) : (
              <ManagerStep
                form={form}
                onForm={(patch) => setForm((f) => ({ ...f, ...patch }))}
                nations={nations}
                playable={playable}
                profiles={profiles}
                clubNation={clubNation}
                onClubNation={changeClubNation}
                club={club}
                onClub={setClub}
                onStart={(id) => void start(id)}
              />
            )}
          </div>
        </div>
      </DialogFrame>
      {busy && <Loading title="Novo Jogo" detail={creation?.label ?? 'A criar o jogo…'} progress={creation?.done ?? 0} />}
    </Overlay>
  )
}
