# NextCM — guia para agentes

Remake do Championship Manager 01/02 (tema Coalizão do patch CLZ) com a base de dados 2025/26.
Core em Rust, desktop em Tauri 2, interface em React + TypeScript + Tailwind, dados em SQLite.

- **Língua:** responder e escrever docs em português do Brasil. Os textos do jogo seguem o CM em
  português de Portugal ("Tácticas", "golo", "equipa", "plantel"): manter esse vocabulário na UI.
- **Visual:** tem de parecer o CM 01/02 (coluna lateral, faixas com as cores do clube, abas escuras,
  botões roxos, diálogos verdes, menus amarelos). Sem animações, cards ou espaçamento de dashboard.
  Os prints que o utilizador mandar são a referência.
- **Detalhe de cada marco** (regras, números, decisões): `ROADMAP.md`. Não leia o arquivo inteiro:
  procure a secção `#### Marco 3.x` que interessa.

## Mapa

| Onde | O quê |
|---|---|
| `core/src/game.rs` (~2000 linhas) | `Game`: save SQLite, dia a dia, dia de jogo (`advance_day` → `live_*` → `live_finish`), notícias, testes de integração no fim |
| `core/src/live.rs` (~1450) | Match Engine v2 por lances, relato, notas, substituições, `LiveView` da tela da partida |
| `core/src/engine.rs` | Força das equipas, golos esperados (base do v2) e penáltis |
| `core/src/regulation.rs`, `core/regulations/*.json` | Regulamentos das competições vivas: fases, participantes, datas por semana ISO (`w14` = fim de semana, `m15` = quarta), subidas e descidas |
| `core/src/game/competitions.rs` | Sorteios, calendários por fase, eliminatórias, passagem de fase, campeões, vistas da Classificação |
| `core/src/game/season.rs` | Fim de época (20/12) e nova época (1/1), contratos, reformas, evolução do CA, juniores |
| `core/src/game/regen.rs` | Jogadores criados pelo jogo (juniores e plantéis dos clubes pequenos) |
| `core/src/finance.rs`, `core/src/game/finances.rs` | Valor de mercado, salário pedido, receitas e despesas de cada clube (livro de contas) |
| `core/src/game/market.rs` | Mercado da IA nas janelas: compras, livres, empréstimos, dispensas, histórico `transfers` |
| `core/src/game/deals.rs` | Mercado do treinador: propostas (`offers`), respostas, contratos, renovações, lista de transferências |
| `core/src/game/unhappy.rs` | Insatisfação do plantel do treinador (motivos, moral) e pedidos para sair |
| `core/src/game/board.rs` | Direção: expectativa, confiança, avisos e despedimento, orçamentos |
| `core/src/tactics.rs` | Formações, escolha automática, `Selection` do treinador, `tactics_from_selection` |
| `core/src/squad.rs` | Condição, moral, lesões e castigos entre jogos |
| `core/src/personality.rs` | Personalidade: as contas dos atributos ocultos (Versatilidade, Adaptabilidade, Desportivismo, Temperamento, Ambição, Lealdade) |
| `core/src/session.rs` | O que o app tem aberto; usado igual pelo Tauri e pela ponte de dev |
| `core/src/bin/sim.rs` | CLI que joga épocas sem UI (calibração) |
| `core/examples/dev_bridge.rs` | Ponte HTTP `POST /invoke/<cmd>` para abrir a UI no navegador |
| `game/src-tauri/src/lib.rs` | Comandos Tauri (cada um delega para `Session`) |
| `game/src/components/MatchDay.tsx` | Dia de jogo e tela da partida ao vivo |
| `game/src/components/TacticsBoard.tsx`, `game/src/tactics.ts` | Lista e campo das Tácticas (tela normal e a meio do jogo) |
| `game/src/bindings/` | Tipos TS **gerados** pelo ts-rs a partir das structs Rust: não editar à mão |
| `scripts/` | Extrator Python dos `.dat` → `data/world.sqlite` |

## Comandos

```powershell
powershell -File scripts/check.ps1          # tudo: fmt, clippy, testes do core, tsc, oxlint
powershell -File scripts/check.ps1 -Quick   # sem os testes do core
cargo test -p nextcm-core --release          # testes (também regenera game/src/bindings)
cargo run -p nextcm-core --release --bin sim -- 7568 epoca 1 20   # 20 épocas: médias de golos, cartões...
npm --prefix game run check                  # só o front: tsc + oxlint
```

Antes de dar uma tarefa por acabada, corra `scripts/check.ps1`.

## Regras

- **Nenhuma regra de jogo no front.** O React só mostra e envia comandos; validações e números ficam no core.
- **Determinismo:** todo o acaso usa `ChaCha8Rng` com a seed do jogo (`engine::match_rng`). Mesma seed =
  mesmo resultado. Coisas só visuais (relato, estatísticas de enchimento) usam um RNG à parte e nunca
  mudam o resultado. Depois de mexer no motor, confira na CLI `sim` que as médias continuam
  (~2,6 golos por jogo, ~48% vitórias em casa).
- **Comando novo** = método em `Game` → `Session` → `game/src-tauri/src/lib.rs` (macro `session_command!`,
  e registar em `generate_handler!`) → `core/examples/dev_bridge.rs` → `game/src/api.ts`.
- **Save:** mudar tabelas exige subir `SAVE_VERSION` em `game.rs` e migrar os saves antigos em `Game::load`.
- Comentários e mensagens de erro em português, curtos, no estilo do código à volta.

## Armadilhas

- **Windows:** corra `cargo` pelo PowerShell. No Git Bash, o `link` do Git atrapalha o linker do MSVC.
- Os tipos em `game/src/bindings/` só se atualizam com `cargo test`: corra-o antes do `tsc` depois de
  mudar structs com `#[ts(export)]`.
- Com a ponte de dev a correr, `dev_bridge.exe` fica bloqueado: pare a ponte antes de a recompilar.
- O jogo aberto vive na memória da ponte: reiniciá-la perde-o (crie outro).
- `oxlint` com `--deny-warnings`: o `react/only-export-components` obriga a separar componentes (`.tsx`)
  de funções e hooks partilhados (`.ts`).

## Ver no navegador

`.claude/launch.json` tem `dev-bridge` (porta 1421) e `game-web` (Vite, 5173). Para chegar depressa a
um dia de jogo, crie o jogo pela ponte a partir da página:

```js
const post = (c, a) => fetch('/invoke/' + c, { method: 'POST', body: JSON.stringify(a ?? {}) }).then(r => r.json())
let info = await post('new_game', { game: { saveName: 'teste ' + Date.now(), firstName: 'A', lastName: 'B', birthDate: null, nationId: null, clubId: 7568 } })
while (!info.live) info = await post('continue_game')   // pára no dia do jogo do Palmeiras
location.reload()                                        // e o Continuar Jogo abre a partida
```

## Commits

Só quando o utilizador pedir. Na `main`, mensagens em português no formato dos anteriores
(`Fase 3, marco 3.x: <título>` e um resumo do que mudou).
