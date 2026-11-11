# 004 — Camisolas a deslizar nas Tácticas

- **Status**: DONE
- **Commit**: c8786e4
- **Severity**: MEDIUM
- **Category**: Missed opportunities
- **Estimated scope**: 2 ficheiros

## Problem

Em `game/src/components/TacticsBoard.tsx:229` cada titular é um `div` com `key={i}` (o lugar) e
`style={{ left: `${s.x}%`, top: `${s.y}%` }}`. Ao trocar jogadores, mudar a formação ou deixar o adjunto
escolher, as camisolas saltam e não se percebe quem foi para onde.

## Target

- `key` pelo jogador: `s.player ? `p${s.player.id}` : `slot${i}``. O mesmo elemento muda de lugar.
- Classe `cm-slide` no marcador:

```css
.cm-slide { transition: left 180ms var(--ease-out), top 180ms var(--ease-out); }
```

  São 11 elementos; `left/top` em percentagem é aceitável aqui e evita reescrever o arrasto.

## Boundaries

- Não mexer na lógica de arrastar e soltar nem nas setas.

## Verification

- **Mechanical**: `powershell -File scripts/check.ps1 -Quick`.
- **Feel check**: mudar de 4-4-2 para 4-3-3: os jogadores deslizam. Arrastar uma camisola não pode ficar atrás
  do rato.
