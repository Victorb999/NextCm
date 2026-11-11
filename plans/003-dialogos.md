# 003 — Entrada dos diálogos verdes

- **Status**: DONE
- **Commit**: c8786e4
- **Severity**: LOW
- **Category**: Missed opportunities
- **Estimated scope**: 2 ficheiros

## Problem

`DialogFrame` (`game/src/components/Dialog.tsx`) aparece de uma vez: o salto de uma tela para um diálogo
grande por cima é brusco.

## Target

Classe `cm-dialog-in` no `div` raiz do `DialogFrame`:

```css
@keyframes cm-dialog-in { from { opacity: 0; transform: scale(0.97); } }
.cm-dialog-in { animation: cm-dialog-in 140ms var(--ease-out); }
```

Origem no centro (é um modal). Fecha sem animação.

## Boundaries

- Não animar os menus de contexto (`CascadeMenu`, `PlayerContextMenu`): são de alta frequência.

## Verification

- **Mechanical**: `powershell -File scripts/check.ps1 -Quick`.
- **Feel check**: abrir as Opções Jogo. Não pode parecer lento (140 ms) nem nascer do nada (começa em 0.97).
