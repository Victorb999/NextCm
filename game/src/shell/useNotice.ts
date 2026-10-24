import { useEffect, useState } from 'react'

/** O aviso amarelo: `notify(msg)` mostra-o e ele some ao fim de 1,8 s. */
export function useNotice() {
  const [notice, setNotice] = useState<string | null>(null)
  useEffect(() => {
    if (!notice) return
    const t = setTimeout(() => setNotice(null), 1800)
    return () => clearTimeout(t)
  }, [notice])
  return { notice, notify: setNotice }
}
