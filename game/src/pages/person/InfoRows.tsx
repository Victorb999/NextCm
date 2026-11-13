import type { ReactNode } from 'react'

/** Linhas "Nome .... valor" de um painel. */
export default function InfoRows({ rows }: { rows: [string, ReactNode][] }) {
  return (
    <div className="cm-zebra">
      {rows.map(([k, v]) => (
        <div key={k} className="flex h-6 items-center justify-between gap-2 px-3 text-[13.5px]">
          <span className="text-cm-muted">{k}</span>
          <span className="truncate font-bold text-cm-yellow">{v}</span>
        </div>
      ))}
    </div>
  )
}
