import type { NewsLink } from '../bindings/NewsLink'
import { useNav } from '../nav'
import { Link } from './Screen'

/** Um texto com os nomes de `links` como ligações (jogador → perfil, clube → clube). */
export default function LinkedText({ text, links }: { text: string; links: NewsLink[] }) {
  const nav = useNav()
  // Corta o texto nos nomes, o mais comprido primeiro (já vêm assim do core)
  const parts: (string | NewsLink)[] = [text]
  for (const l of links) {
    for (let i = 0; i < parts.length; i++) {
      const p = parts[i]
      if (typeof p !== 'string') continue
      const at = p.indexOf(l.text)
      if (at < 0) continue
      parts.splice(i, 1, p.slice(0, at), l, p.slice(at + l.text.length))
      i++
    }
  }
  return (
    <>
      {parts.map((p, i) =>
        typeof p === 'string' ? (
          p
        ) : (
          <Link key={i} className="text-cm-yellow" onClick={() => nav.go(p.kind === 'club' ? { kind: 'club', id: p.id } : { kind: 'person', id: p.id })}>
            {p.text}
          </Link>
        ),
      )}
    </>
  )
}
