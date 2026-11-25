import { useState } from 'react'
import changelog from '../../../CHANGELOG.md?raw'
import { DialogFrame, Overlay } from './Dialog'

/** As versões do CHANGELOG.md: título (## ...) e os itens (- ...). */
function parseChangelog(md: string) {
  const versions: { title: string; items: string[] }[] = []
  for (const line of md.split(/\r?\n/)) {
    if (line.startsWith('## ')) versions.push({ title: line.slice(3).trim(), items: [] })
    else if (line.startsWith('- ')) versions.at(-1)?.items.push(line.slice(2).trim())
  }
  return versions
}

/** Texto com **negrito**. */
function Rich({ text }: { text: string }) {
  return (
    <>
      {text.split(/\*\*(.+?)\*\*/).map((part, i) =>
        i % 2 ? (
          <b key={i} className="text-white">
            {part}
          </b>
        ) : (
          part
        ),
      )}
    </>
  )
}

const TABS = ['Versão', 'Novidades', 'Créditos'] as const

/** Sobre o NextCM: versão, novidades (CHANGELOG.md) e créditos. */
export default function AboutDialog({ onClose }: { onClose: () => void }) {
  const [tab, setTab] = useState<(typeof TABS)[number]>('Versão')
  return (
    <Overlay>
      <DialogFrame title="Sobre o NextCM" width={620} onOk={onClose} okLabel="Fechar">
        <div className="mb-2 flex gap-1">
          {TABS.map((t) => (
            <button
              key={t}
              onClick={() => setTab(t)}
              className={`h-8 flex-1 rounded font-title text-[15px] ${t === tab ? 'bg-cm-purple text-cm-yellow' : 'bg-black/40 hover:bg-black/70'}`}
            >
              {t}
            </button>
          ))}
        </div>
        <div className="h-[340px] overflow-auto pr-1 text-[13px] leading-5">
          {tab === 'Versão' && (
            <div className="space-y-3">
              <div className="font-title text-[26px] text-cm-yellow">NextCM {__APP_VERSION__}</div>
              <p>Remake de fãs do Championship Manager 01/02, com a base de dados 2025/26.</p>
              <p>
                Site: <span className="text-cm-yellow">victorb999.github.io/NextCm</span>
              </p>
              <p>
                Encontrou um erro? Abra uma Issue no GitHub (Victorb999/NextCm) com a versão, os passos e o arquivo de log, que fica em{' '}
                <span className="text-cm-yellow">%LOCALAPPDATA%\com.nextcm.app\logs\NextCM.log</span>.
              </p>
            </div>
          )}
          {tab === 'Novidades' &&
            parseChangelog(changelog).map((v) => (
              <div key={v.title} className="mb-3">
                <div className="font-title text-[17px] text-cm-yellow">{v.title}</div>
                <ul className="ml-4 list-disc">
                  {v.items.map((it) => (
                    <li key={it}>
                      <Rich text={it} />
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          {tab === 'Créditos' && (
            <div className="space-y-3">
              <p>
                A base de dados 2025/26 e o tema vêm do <b className="text-white">Update Coalizão</b>, da equipe{' '}
                <b className="text-white">Coalizão CM</b> (coalizaocm.com.br), o maior acervo nacional de CM 01/02. Sem o trabalho deles,
                este jogo não teria elencos.
              </p>
              <p>Feito em Rust, Tauri, React e SQLite. Projeto aberto e sem fins comerciais.</p>
              <p className="text-cm-dim">
                Championship Manager é uma marca dos seus donos. O NextCM é um projeto de fãs independente, sem ligação à Sports
                Interactive, à Eidos ou à Square Enix.
              </p>
            </div>
          )}
        </div>
      </DialogFrame>
    </Overlay>
  )
}
