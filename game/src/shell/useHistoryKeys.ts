import { useEffect } from 'react'
import { useNav } from '../nav'

/** Alt+← / Alt+→ e os botões "voltar"/"avançar" do mouse navegam no histórico de telas. */
export function useHistoryKeys() {
  const nav = useNav()
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.altKey && e.key === 'ArrowLeft') nav.back()
      if (e.altKey && e.key === 'ArrowRight') nav.forward()
    }
    const onMouse = (e: MouseEvent) => {
      if (e.button === 3) nav.back()
      if (e.button === 4) nav.forward()
    }
    window.addEventListener('keydown', onKey)
    window.addEventListener('mouseup', onMouse)
    return () => {
      window.removeEventListener('keydown', onKey)
      window.removeEventListener('mouseup', onMouse)
    }
  }, [nav])
}
