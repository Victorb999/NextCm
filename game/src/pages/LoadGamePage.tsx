import { useEffect, useState } from 'react'
import { api } from '../api'
import type { SaveSummary } from '../bindings/SaveSummary'
import { DialogFrame, Overlay } from '../components/Dialog'
import { cmDate } from '../format'
import { useGame } from '../game'
import { useNav } from '../nav'

const savedAt = (secs: number) => {
  if (!secs) return '-'
  const d = new Date(secs * 1000)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getDate()}.${d.getMonth() + 1}.${d.getFullYear()} ${pad(d.getHours())}:${pad(d.getMinutes())}`
}

/** "Carregar Jogo": lista dos saves; duplo clique ou Ok abre. */
export default function LoadGamePage() {
  const game = useGame()
  const nav = useNav()
  const [saves, setSaves] = useState<SaveSummary[] | null>(null)
  const [selected, setSelected] = useState(0)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    api.listSaves().then(setSaves, (e) => setError(String(e)))
  }, [])

  const load = async (s: SaveSummary | undefined) => {
    if (!s || busy) return
    setBusy(true)
    try {
      game.update(await api.loadGame(s.file))
      nav.go({ kind: 'news' })
    } catch (e) {
      setError(String(e))
      setBusy(false)
    }
  }

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') game.setMode('start')
      if (e.key === 'Enter') void load(saves?.[selected])
      if (e.key === 'ArrowDown') setSelected((i) => Math.min((saves?.length ?? 1) - 1, i + 1))
      if (e.key === 'ArrowUp') setSelected((i) => Math.max(0, i - 1))
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  })

  const head = ['Jogo', 'Treinador', 'Clube', 'Data do jogo', 'Gravado em']
  return (
    <Overlay>
      <DialogFrame
        title="Carregar Jogo"
        width={760}
        onCancel={() => game.setMode('start')}
        onOk={saves?.length ? () => void load(saves[selected]) : undefined}
        okLabel={busy ? 'A carregar…' : 'Ok'}
      >
        {error && <div className="mb-2 bg-red-900 px-2 py-1 text-white">{error}</div>}
        {saves && saves.length === 0 && <div className="p-6 text-center text-cm-yellow">Nenhum jogo gravado.</div>}
        {saves && saves.length > 0 && (
          <table className="w-full border-collapse text-[14px]">
            <thead>
              <tr className="h-6">
                {head.map((h) => (
                  <th key={h} className="border border-black bg-cm-head px-2 text-left font-normal text-white">
                    {h}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {saves.map((s, i) => (
                <tr
                  key={s.file}
                  onMouseDown={() => setSelected(i)}
                  onDoubleClick={() => void load(s)}
                  className={`h-7 ${i === selected ? 'bg-cm-sel' : 'bg-black/50'}`}
                >
                  <td className="px-2 font-bold text-white">{s.name}</td>
                  <td className="px-2 text-cm-pink">{s.managerName}</td>
                  <td className="px-2 text-cm-magenta">{s.clubName}</td>
                  <td className="px-2 text-cm-yellow">{cmDate(s.date)}</td>
                  <td className="px-2 text-[#aab3c4]">{savedAt(s.savedAt)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </DialogFrame>
    </Overlay>
  )
}
