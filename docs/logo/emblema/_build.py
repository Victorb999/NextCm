"""Gera os SVGs do logo NextCM (direção Emblema). Letras desenhadas à mão, sem fontes.

python docs/logo/emblema/_build.py
"""

import math
from pathlib import Path

OUT = Path(__file__).parent

ROXO = "#4A1FC9"
VERDE = "#1ED67A"
SOMBRA = "#1E0866"
FUNDO = "#140A2E"
BRANCO = "#FFFFFF"

# ---------------------------------------------------------------------------
# Alfabeto: altura de maiúscula 160, x-height 120 (topo em y=40), linha de base y=160.
# Haste 34, curvas exteriores raio 28–40, cantos interiores retos (estilo números de camisola).
G = {
    "N": (120, "M0 160V0H34L86 96V0H120V160H86L34 64V160Z"),
    "E": (100, "M0 0H100V34H34V62H88V96H34V126H100V160H0Z"),
    "X": (124, "M0 0H38L62 52L86 0H124L82 80L124 160H86L62 108L38 160H0L42 80Z"),
    "T": (110, "M0 0H110V34H72V160H38V34H0Z"),
    "C": (112, "M40 0H112V36H42A8 8 0 0 0 34 44V116A8 8 0 0 0 42 124H112V160H40A40 40 0 0 1 0 120V40A40 40 0 0 1 40 0Z"),
    "M": (150, "M0 160V0H40L75 84L110 0H150V160H116V64L88 130H62L34 64V160Z"),
    "e": (100, "M100 112V68A28 28 0 0 0 72 40H28A28 28 0 0 0 0 68V132A28 28 0 0 0 28 160H100V128H34V112Z"
               "M34 70H66V92H34Z"),
    "x": (100, "M0 40H36L50 80L64 40H100L70 100L100 160H64L50 120L36 160H0L30 100Z"),
    "t": (72, "M14 10H48V40H72V68H48V126H72V160H42A28 28 0 0 1 14 132V68H0V40H14Z"),
    "0": (92, "M30 0H62A30 30 0 0 1 92 30V130A30 30 0 0 1 62 160H30A30 30 0 0 1 0 130V30A30 30 0 0 1 30 0Z"
              "M34 34V126H58V34Z"),
    "1": (56, "M0 0H50V160H16V34H0Z"),
    "2": (90, "M0 0H62A28 28 0 0 1 90 28V70A28 28 0 0 1 62 98H34V126H90V160H0V90A28 28 0 0 1 28 62H56V34H0Z"),
    "5": (90, "M0 0H90V34H34V62H62A28 28 0 0 1 90 90V132A28 28 0 0 1 62 160H0V126H56V96H0Z"),
    "6": (90, "M28 0H90V34H34V62H62A28 28 0 0 1 90 90V132A28 28 0 0 1 62 160H28A28 28 0 0 1 0 132V28A28 28 0 0 1 28 0Z"
              "M34 96V126H56V96Z"),
    "/": (64, "M40 0H64L24 160H0Z"),
    "·": (34, "M0 63H34V97H0Z"),
}
# Espaçamento óptico por par (depois da letra da esquerda)
KERN = {"Ne": 10, "ex": 6, "xt": 4, "tC": 16, "CM": 10}


def word(text, gap=10, kern=None):
    """Devolve (largura, [(x, path)]) com a linha de base em y=160."""
    kern = kern or {}
    x, parts = 0.0, []
    for i, ch in enumerate(text):
        if ch == " ":
            x += 40
            continue
        w, d = G[ch]
        parts.append((x, d))
        x += w
        if i + 1 < len(text):
            x += kern.get(ch + text[i + 1], gap)
    return x, parts


def word_svg(text, fill, gap=10, kern=None, skew=0.0, extra=""):
    w, parts = word(text, gap, kern)
    sk = f" translate(0 160) skewX({skew}) translate(0 -160)" if skew else ""
    g = "".join(f'<path transform="translate({x:g} 0)" d="{d}"/>' for x, d in parts)
    return w, f'<g fill="{fill}"{extra} transform="{sk.strip()}">{g}</g>' if skew else f'<g fill="{fill}"{extra}>{g}</g>'


def star(cx, cy, r, ratio=0.45):
    pts = []
    for i in range(10):
        a = math.radians(-90 + 36 * i)
        rr = r if i % 2 == 0 else r * ratio
        pts.append(f"{cx + rr * math.cos(a):.1f},{cy + rr * math.sin(a):.1f}")
    return " ".join(pts)


def n_mark(cx, cy, h, fill):
    s = h / 160
    return f'<path transform="translate({cx - 60 * s:.2f} {cy - 80 * s:.2f}) scale({s:.4f})" d="{G["N"][1]}" fill="{fill}"/>'


def arc_text(text, r, size, fill, bottom=False, gap=14):
    """Letras construídas ao longo de um círculo centrado na origem."""
    s = size / 160
    w, parts = word(text, gap)
    total = w * s
    out = []
    for x, d in parts:
        ch_w = next(G[c][0] for c in G if G[c][1] == d)
        mid = (x + ch_w / 2) * s - total / 2  # distância ao centro do arco
        ang = math.degrees(mid / r)
        if not bottom:
            # base no raio r, letras para fora; esquerda→direita = ângulo crescente
            t = f"rotate({ang:.3f}) translate({-ch_w * s / 2:.2f} {-r - size:.2f}) scale({s:.4f})"
        else:
            # topo no raio r, letras para fora (de pé para quem lê em baixo)
            t = f"rotate({-ang:.3f}) translate({-ch_w * s / 2:.2f} {r:.2f}) scale({s:.4f})"
        out.append(f'<path transform="{t}" d="{d}"/>')
    return f'<g fill="{fill}">{"".join(out)}</g>'


def svg(vb, body, title="NextCM"):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{vb}">\n'
            f"  <title>{title}</title>\n  {body}\n</svg>\n")


def write(name, content):
    (OUT / name).write_text(content, encoding="utf-8")
    print("wrote", name)


# ---------------------------------------------------------------------------
# Símbolo: estrela de campeão + moeda (anel branco, faixa roxa, anel verde, centro escuro, N).
def badge(cx, cy, R, small=False):
    if small:  # corte para 16–32 px: sem o anel verde, traços mais grossos, N maior
        return (f'<circle cx="{cx}" cy="{cy}" r="{R}" fill="{ROXO}"/>'
                f'<circle cx="{cx}" cy="{cy}" r="{R * 0.86:.2f}" fill="none" stroke="{BRANCO}" stroke-width="{R * 0.14:.2f}"/>'
                + n_mark(cx, cy, R * 0.98, BRANCO))
    return (f'<circle cx="{cx}" cy="{cy}" r="{R}" fill="{ROXO}"/>'
            f'<circle cx="{cx}" cy="{cy}" r="{R * 0.93:.2f}" fill="none" stroke="{BRANCO}" stroke-width="{R * 0.08:.2f}"/>'
            f'<circle cx="{cx}" cy="{cy}" r="{R * 0.70:.2f}" fill="{SOMBRA}" stroke="{VERDE}" stroke-width="{R * 0.06:.2f}"/>'
            + n_mark(cx, cy, R * 0.80, BRANCO))


simbolo = (f'<polygon points="{star(128, 30, 26)}" fill="{VERDE}"/>' + badge(128, 152, 98))
write("simbolo.svg", svg("0 0 256 256", simbolo, "NextCM — símbolo"))
simbolo_p = (f'<polygon points="{star(128, 30, 28, 0.5)}" fill="{VERDE}"/>' + badge(128, 154, 96, small=True))
write("simbolo-pequeno.svg", svg("0 0 256 256", simbolo_p, "NextCM — símbolo (tamanhos pequenos)"))

# Ícone da app: o símbolo sobre o azulejo escuro (a moeda roxa destaca-se).
def icone(inner_svg_body, scale=0.80):
    off = 128 - 128 * scale
    return (f'<rect width="256" height="256" rx="56" fill="{FUNDO}"/>'
            f'<g transform="translate({off:.1f} {off + 2:.1f}) scale({scale})">{inner_svg_body}</g>')


write("icone-app.svg", svg("0 0 256 256", icone(simbolo), "NextCM — ícone"))
write("icone-app-pequeno.svg", svg("0 0 256 256", icone(simbolo_p, 0.96), "NextCM — ícone (pequeno)"))

# Favicon (16–48 px): só a moeda, a ocupar o quadrado; a estrela some a esse tamanho.
write("favicon.svg", svg("0 0 256 256", badge(128, 128, 126, small=True), "NextCM"))


# Uma cor: anel, centro e N como furos de verdade (evenodd), para carimbos, bordados, gravação.
def circ(cx, cy, r):
    return f"M{cx - r:.2f} {cy}a{r:.2f} {r:.2f} 0 1 0 {2 * r:.2f} 0a{r:.2f} {r:.2f} 0 1 0 {-2 * r:.2f} 0Z"


def mono(cor):
    cx, cy, R = 128, 152, 98
    h = R * 0.80
    s = h / 160
    import re

    out, x, y = [], 0.0, 0.0
    for cmd, args in re.findall(r"([MVHLZ])([^MVHLZ]*)", G["N"][1]):
        a = [float(v) for v in re.findall(r"-?\d+\.?\d*", args)]
        if cmd in "ML":
            x, y = a
        elif cmd == "V":
            y = a[0]
        elif cmd == "H":
            x = a[0]
        if cmd != "Z":
            out.append(f"{'M' if cmd == 'M' else 'L'}{cx - 60 * s + x * s:.2f} {cy - 80 * s + y * s:.2f}")
    n_path = "".join(out) + "Z"
    d = circ(cx, cy, R) + circ(cx, cy, R * 0.84) + circ(cx, cy, R * 0.74) + n_path
    return (f'<polygon points="{star(128, 30, 26)}" fill="{cor}"/>'
            f'<path fill-rule="evenodd" fill="{cor}" d="{d}"/>')


write("simbolo-preto.svg", svg("0 0 256 256", mono("#000000"), "NextCM — símbolo (uma cor)"))
write("simbolo-branco.svg", svg("0 0 256 256", mono(BRANCO), "NextCM — símbolo (uma cor, fundo escuro)"))
write("simbolo-roxo.svg", svg("0 0 256 256", mono(ROXO), "NextCM — símbolo (roxo)"))

# ---------------------------------------------------------------------------
# Wordmark "NextCM": inclinado 12°, "Next" numa cor e "CM" a verde.
def wordmark(fill_next, fill_cm, shadow=None, skew=-12):
    w, parts = word("NextCM", kern=KERN)
    sk = f"translate(0 160) skewX({skew}) translate(0 -160)"
    def layer(dx, dy, fn, fc):
        g = []
        for i, (x, d) in enumerate(parts):
            g.append(f'<path transform="translate({x + dx:g} {dy})" d="{d}" fill="{fn if i < 4 else fc}"/>')
        return "".join(g)
    body = (layer(10, 10, shadow, shadow) if shadow else "") + layer(0, 0, fill_next, fill_cm)
    return w, f'<g transform="{sk}">{body}</g>'


ww, wm = wordmark(FUNDO, ROXO)
pad = 40
write("wordmark.svg", svg(f"-10 -4 {ww + 24:.0f} 168", wm, "NextCM — wordmark"))
ww2, wm_esc = wordmark(BRANCO, VERDE)
write("wordmark-negativo.svg", svg(f"-10 -4 {ww2 + 24:.0f} 168", wm_esc, "NextCM — wordmark (fundo escuro)"))

# Horizontal: símbolo (256 de altura) + wordmark com altura de maiúscula = 0,5 do símbolo
def horizontal(fn, fc):
    s = 0.62
    w, wmk = wordmark(fn, fc)
    x0 = 256 + 40
    return f"0 0 {x0 + w * s + 24:.0f} 256", simbolo + f'<g transform="translate({x0} {152 - 80 * s:.1f}) scale({s})">{wmk}</g>'


vb, b = horizontal(FUNDO, ROXO)
write("horizontal.svg", svg(vb, b, "NextCM — horizontal"))
vb, b = horizontal(BRANCO, VERDE)
write("horizontal-negativo.svg", svg(vb, b, "NextCM — horizontal (fundo escuro)"))

# Empilhado
def empilhado(fn, fc):
    s = 0.5
    w, wmk = wordmark(fn, fc)
    W = max(256, w * s + 30)
    return (f"0 0 {W:.0f} 352",
            f'<g transform="translate({(W - 256) / 2:.1f} 0)">{simbolo}</g>'
            f'<g transform="translate({(W - w * s) / 2 + 8:.1f} 270) scale({s})">{wmk}</g>')


vb, b = empilhado(FUNDO, ROXO)
write("empilhado.svg", svg(vb, b, "NextCM — empilhado"))
vb, b = empilhado(BRANCO, VERDE)
write("empilhado-negativo.svg", svg(vb, b, "NextCM — empilhado (fundo escuro)"))

# ---------------------------------------------------------------------------
# Emblema completo (tela inicial): anel com NEXTCM e 01/02 · 25/26, três estrelas, sombra.
def emblema_completo():
    R = 196
    return (f'<g transform="translate(250 262)">'
            f'<circle cx="8" cy="8" r="{R}" fill="{SOMBRA}"/>'
            f'<circle r="{R}" fill="{ROXO}"/>'
            f'<circle r="{R - 12}" fill="none" stroke="{BRANCO}" stroke-width="10"/>'
            f'<circle r="132" fill="{SOMBRA}" stroke="{VERDE}" stroke-width="8"/>'
            + arc_text("NEXTCM", 148, 30, BRANCO, gap=30)
            + arc_text("01/02 · 25/26", 146, 26, VERDE, bottom=True, gap=16)
            + f'<circle cx="-158" r="6" fill="{VERDE}"/><circle cx="158" r="6" fill="{VERDE}"/>'
            + n_mark(0, 0, 132, BRANCO)
            + f'<polygon points="{star(-64, -214, 20)}" fill="{VERDE}"/>'
            f'<polygon points="{star(0, -232, 28)}" fill="{VERDE}"/>'
            f'<polygon points="{star(64, -214, 20)}" fill="{VERDE}"/></g>')


em = emblema_completo()
write("emblema-completo.svg", svg("44 16 412 460", em, "NextCM — emblema completo"))

w, wmk = wordmark(BRANCO, VERDE, shadow=SOMBRA)
s = 0.95
tela = em + f'<g transform="translate(500 {262 - 80 * s:.0f}) scale({s})">{wmk}</g>'
write("tela-inicial.svg", svg(f"40 16 {500 + w * s + 40:.0f} 460", tela, "NextCM — tela inicial"))
write("tela-inicial-preview.svg", svg(f"0 0 {500 + w * s + 90:.0f} 500",
      f'<rect width="{500 + w * s + 90:.0f}" height="500" fill="{FUNDO}"/>' + tela, "NextCM — tela inicial (com fundo)"))
