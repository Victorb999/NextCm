"""Opções do N com relevo (a partir do desenho do utilizador). python docs/logo/n-relevo/_build.py"""

from pathlib import Path

OUT = Path(__file__).parent
ROXO, VERDE, VERDE2, ESCURO, BRANCO = "#4F0BB4", "#1DD679", "#12A35A", "#1D0865", "#FFFFFF"

# N limpo: caixa 120×160, hastes 32, diagonal a 45° de canto a canto (ponta em cima à esquerda e
# em baixo à direita, como no original), bordas da diagonal paralelas.
N = [(0, 0), (88, 88), (88, 0), (120, 0), (120, 160), (32, 72), (32, 160), (0, 160)]

# Letras do "extCM" (altura de maiúscula 160, x-height 120, hastes 32), cortes retos como o N.
G = {
    "e": (100, "M100 112V68A28 28 0 0 0 72 40H28A28 28 0 0 0 0 68V132A28 28 0 0 0 28 160H100V128H32V112Z"
               "M32 70H68V92H32Z"),
    "x": (104, "M0 40H38L52 76L66 40H104L72 100L104 160H66L52 124L38 160H0L32 100Z"),
    "t": (72, "M16 8H48V40H72V70H48V128H72V160H44A28 28 0 0 1 16 132V70H0V40H16Z"),
    "C": (112, "M40 0H112V34H40A6 6 0 0 0 34 40V120A6 6 0 0 0 40 126H112V160H40A40 40 0 0 1 0 120V40A40 40 0 0 1 40 0Z"),
    "M": (148, "M0 160V0H36L74 80L112 0H148V160H116V64L86 128H62L32 64V160Z"),
}


def pts(p, dx=0.0, dy=0.0, s=1.0):
    return " ".join(f"{x * s + dx:.2f},{y * s + dy:.2f}" for x, y in p)


def relevo(v, face, sombra=None, dx=0.0, dy=0.0, s=1.0, fill_n=BRANCO):
    """N extrudido pelo vetor v: a face de trás e um quadrilátero por aresta (a varredura do N),
    depois o N por cima. Com `sombra`, as faces horizontais/diagonais ficam num verde mais escuro."""
    vx, vy = v
    out = [f'<polygon points="{pts(N, dx + vx * s, dy + vy * s, s)}" fill="{face}"/>']
    for i in range(len(N)):
        (x1, y1), (x2, y2) = N[i], N[(i + 1) % len(N)]
        quad = [(x1, y1), (x2, y2), (x2 + vx, y2 + vy), (x1 + vx, y1 + vy)]
        cor = face if (sombra is None or x1 == x2) else sombra
        out.append(f'<polygon points="{pts(quad, dx, dy, s)}" fill="{cor}"/>')
    out.append(f'<polygon points="{pts(N, dx, dy, s)}" fill="{fill_n}"/>')
    return "".join(out)


def svg(vb, body, title):
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{vb}">\n  <title>{title}</title>\n  {body}\n</svg>\n'


def write(name, s):
    (OUT / name).write_text(s, encoding="utf-8")


OPCOES = {
    # A: o relevo para cima e para a esquerda, a 45° como a diagonal — a diagonal fica limpa,
    # as faces verdes só aparecem nas hastes (lê-se como uma fita).
    "a-fita": dict(v=(-14, -14), face=VERDE),
    # B: a leitura do original, mas coerente: tudo para baixo e para a esquerda, a diagonal mostra
    # a face de baixo, as hastes a da esquerda.
    "b-relevo": dict(v=(-14, 14), face=VERDE),
    # C: bloco fundo (tela inicial, capa): para baixo e para a direita, duas sombras de verde.
    "c-bloco": dict(v=(18, 26), face=VERDE, sombra=VERDE2),
}


def caixa(v, s):
    vx, vy = v
    x0, x1 = min(0, vx) * s, (120 + max(0, vx)) * s
    y0, y1 = min(0, vy) * s, (160 + max(0, vy)) * s
    return x0, y0, x1 - x0, y1 - y0


for nome, o in OPCOES.items():
    v = o["v"]
    # Símbolo sozinho (256): centrado pela caixa total, 2% acima do centro.
    s = 1.05
    x0, y0, w, h = caixa(v, s)
    dx, dy = 128 - w / 2 - x0, 124 - h / 2 - y0
    write(f"{nome}.svg", svg("0 0 256 256", relevo(dx=dx, dy=dy, s=s, **o), f"NextCM — {nome}"))
    # Ícone: azulejo roxo chapado.
    s = 0.92
    x0, y0, w, h = caixa(v, s)
    dx, dy = 128 - w / 2 - x0, 125 - h / 2 - y0
    write(f"{nome}-icone.svg", svg("0 0 256 256",
          f'<rect width="256" height="256" rx="56" fill="{ROXO}"/>' + relevo(dx=dx, dy=dy, s=s, **o),
          f"NextCM — {nome} (ícone)"))

    # Lockup: o N na mesma altura das maiúsculas do "CM", "ext" em minúsculas, mesma linha de base.
    def lockup(fundo, cor_txt, cor_cm, fill_n):
        vx, vy = v
        x = 120 + max(0, vx) + 24
        letras = []
        gap = {"e": 12, "x": 10, "t": 18, "C": 12}
        for ch in "extCM":
            w_, d = G[ch]
            letras.append(f'<path transform="translate({x} 0)" d="{d}" fill="{cor_cm if ch in "CM" else cor_txt}"/>')
            x += w_ + gap.get(ch, 0)
        x0_ = min(0, vx) - 30
        y0_ = min(0, vy) - 30
        W = x - x0_ + 30
        H = 160 + abs(vy) + 60
        bg = f'<rect x="{x0_}" y="{y0_}" width="{W}" height="{H}" rx="24" fill="{fundo}"/>' if fundo else ""
        return svg(f"{x0_} {y0_} {W} {H}", bg + relevo(fill_n=fill_n, **{k: o[k] for k in o}) + "".join(letras),
                   f"NextCM — {nome} (horizontal)")

    write(f"{nome}-horizontal-escuro.svg", lockup(ESCURO, BRANCO, VERDE, BRANCO))
    write(f"{nome}-horizontal-claro.svg", lockup(None, ESCURO, ROXO, ROXO))
print("ok")
