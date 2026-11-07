import { useEffect, useState } from 'react'
import { api } from '../api'
import { useGame } from '../game'
import { useNav } from '../nav'
import { personActions, setPendingAct } from '../personActions'
import CascadeMenu, { type MenuNode } from './CascadeMenu'

/**
 * Botão direito num jogador de uma lista (como no FM): as acções do menu Acção do perfil,
 * num menu ali mesmo. As linhas marcam-se com `data-person={id}`. A acção escolhida abre o
 * perfil e corre lá (os diálogos são os do perfil).
 */
export default function PlayerContextMenu() {
  const nav = useNav()
  const game = useGame()
  const [menu, setMenu] = useState<{ x: number; y: number; items: MenuNode[] } | null>(null)
  const playing = game.mode === 'game'
  const club = game.info?.club.id

  useEffect(() => {
    const onMenu = (e: MouseEvent) => {
      const row = (e.target as Element).closest<HTMLElement>('[data-person]')
      const id = Number(row?.dataset.person)
      if (!row || !id) return
      e.preventDefault()
      const { clientX: x, clientY: y } = e
      void (async () => {
        try {
          const person = await api.getPerson(id)
          const market = playing && person.player ? await api.transferView(id).catch(() => null) : null
          const open = () => nav.go({ kind: 'person', id })
          const run = (key: string) => () => {
            setPendingAct(id, key)
            open()
          }
          const groups =
            playing && person.player && !person.ghost
              ? personActions({ person, market, mine: person.club?.id === club, playing })
              : []
          const items: MenuNode[] = [
            { label: `Ver Perfil: ${person.name}`, onSelect: open },
            ...groups.flatMap((g): MenuNode[] => ['separator', ...g.map((a) => ({ label: a.label, onSelect: run(a.key) }))]),
          ]
          setMenu({ x, y, items })
        } catch {
          setMenu(null)
        }
      })()
    }
    document.addEventListener('contextmenu', onMenu)
    return () => document.removeEventListener('contextmenu', onMenu)
  }, [nav, playing, club])

  return menu && <CascadeMenu items={menu.items} x={menu.x} y={menu.y} onClose={() => setMenu(null)} />
}
