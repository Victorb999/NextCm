# 005 — Golo e relato

- **Status**: DONE
- **Commit**: c8786e4
- **Severity**: MEDIUM
- **Category**: Easing & duration / Missed opportunities
- **Estimated scope**: 2 ficheiros

## Problem

`game/src/components/MatchDay.tsx:270`, no `LineCard`: `transition-colors duration-300 ease-in-out`. Corre a
cada frase (de 60 ms a 1,9 s), com `ease-in-out`, e o fundo em gradiente nem transita. O golo, o momento alto,
não tem destaque além da cor. O placar (`ScoreBand`) muda de número sem sinal.

## Target

- Tirar `transition-colors duration-300 ease-in-out` do `LineCard`.
- Frase nova: `key` por frase e a classe `cm-line-in`:
  `@keyframes cm-line-in { from { opacity: 0; transform: translateY(4px); } }`,
  `.cm-line-in { animation: cm-line-in 100ms var(--ease-out); }`.
- Frase do golo: `cm-goal-in`:
  `@keyframes cm-goal-in { from { transform: scale(1.04); filter: brightness(1.6); } }`, 320 ms ease-out.
- Placar: o número com `key={goals}` e `cm-score-pop`:
  `@keyframes cm-score-pop { 40% { transform: scale(1.18); } }`, 260 ms ease-out.

## Boundaries

- Não mexer no ritmo do relato nem no core.

## Verification

- **Mechanical**: `powershell -File scripts/check.ps1 -Quick`.
- **Feel check**: um jogo em Muito Rápida; as frases não podem parecer atrasadas. Num golo, o cartão acende e o
  placar dá um pulo.
