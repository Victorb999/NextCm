# Planos de animação e som (item 24 do 3.19.1)

Auditoria de 11/11/2026 (commit c8786e4). O front quase não tinha movimento; o trabalho é acrescentar
pouco e curto, sem fugir do CM 01/02. Não há transições entre telas nem nos menus de contexto: são as acções
mais frequentes e o CM troca de tela na hora.

| # | Plano | Depende de | Estado |
|---|---|---|---|
| 001 | [Interruptores Animações e Som](001-interruptores.md) | — | DONE |
| 002 | [Botões afundam ao clicar](002-clique-nos-botoes.md) | 001 | DONE |
| 003 | [Entrada dos diálogos verdes](003-dialogos.md) | 001 | DONE |
| 004 | [Camisolas a deslizar nas Tácticas](004-tacticas-a-deslizar.md) | 001 | DONE |
| 005 | [Golo e relato](005-golo-e-relato.md) | 001 | DONE |
| 006 | [Sons sintetizados da partida](006-sons-da-partida.md) | 001 | DONE |

Ordem: 001 primeiro (os outros usam `data-motion` e `settings.sound`), depois qualquer ordem.

Valores comuns: `--ease-out: cubic-bezier(0.23, 1, 0.32, 1)`. Tudo desliga com `data-motion="off"` (Opções)
e com `prefers-reduced-motion: reduce`.
