import type { NewsLink } from '../bindings/NewsLink'
import { localText } from '../format'
import { useNav } from '../nav'
import { Link } from './Screen'

const LETTER = /[\p{L}\p{N}]/u

/** Onde `name` aparece em `text` como palavra inteira ("Inter" não apanha "Interesse"). */
function findWord(text: string, name: string): number {
  for (let at = text.indexOf(name); at >= 0; at = text.indexOf(name, at + 1)) {
    const before = text[at - 1]
    const after = text[at + name.length]
    if ((!before || !LETTER.test(before)) && (!after || !LETTER.test(after))) return at
  }
  return -1
}

/** Um texto com os nomes de `links` como ligações (jogador → perfil, clube → clube). */
export default function LinkedText({ text, links }: { text: string; links: NewsLink[] }) {
  const nav = useNav()
  // Corta o texto nos nomes, o mais comprido primeiro (já vêm assim do core); os valores na
  // moeda e no período de salários das Definições
  const parts: (string | NewsLink)[] = [localText(text)]
  for (const l of links) {
    for (let i = 0; i < parts.length; i++) {
      const p = parts[i]
      if (typeof p !== 'string') continue
      const at = findWord(p, l.text)
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
          <Link
            key={i}
            className="text-cm-yellow"
            onClick={() => nav.go(p.kind === 'club' ? { kind: 'club', id: p.id } : { kind: 'person', id: p.id })}
          >
            {p.text}
          </Link>
        ),
      )}
    </>
  )
}
