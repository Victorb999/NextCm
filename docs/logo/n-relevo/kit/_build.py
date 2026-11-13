"""Kit do logo NextCM — o N do utilizador com a geometria limpa. python docs/logo/n-relevo/kit/_build.py"""

import importlib.util
from pathlib import Path

OUT = Path(__file__).parent
_spec = importlib.util.spec_from_file_location("base", OUT.parent / "_build.py")
base = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(base)  # também regenera as opções na pasta de cima

N, G, svg = base.N, base.G, base.svg
ROXO, VERDE, ESCURO, BRANCO = base.ROXO, base.VERDE, base.ESCURO, base.BRANCO


def relevo(v, face=VERDE, dx=0.0, dy=0.0, s=1.0, fill_n=BRANCO, **_):
    """O N do utilizador, com a geometria limpa: as faces verdes à esquerda das hastes sobem a 45°
    (sem tampa em cima da segunda haste) e a da diagonal cai por baixo dela, até à ponta."""
    d = -v[0]
    faces = [
        [(0, 0), (0, 160), (-d, 160 - d), (-d, -d)],      # esquerda da 1.ª haste
        [(88, 0), (88, 88), (88 - d, 88 - d), (88 - d, -d)],  # esquerda da 2.ª haste, até à diagonal
        [(32, 72), (120, 160), (120, 160 + d), (32, 72 + d)],  # por baixo da diagonal
    ]
    out = [f'<polygon points="{base.pts(f, dx, dy, s)}" fill="{face}"/>' for f in faces]
    out.append(f'<polygon points="{base.pts(N, dx, dy, s)}" fill="{fill_n}"/>')
    return "".join(out)

V = (-14, -14)       # relevo normal: para cima e para a esquerda, a 45° (paralelo à diagonal)
V_PEQ = (-22, -22)   # corte pequeno (16–48 px): relevo mais fundo para o verde não sumir

# Algarismos para a data da tela inicial (mesmo desenho das letras).
G.update({
    "0": (92, "M30 0H62A30 30 0 0 1 92 30V130A30 30 0 0 1 62 160H30A30 30 0 0 1 0 130V30A30 30 0 0 1 30 0Z"
              "M32 34V126H60V34Z"),
    "1": (56, "M0 0H50V160H18V32H0Z"),
    "2": (90, "M0 0H62A28 28 0 0 1 90 28V70A28 28 0 0 1 62 98H32V128H90V160H0V90A28 28 0 0 1 28 62H58V32H0Z"),
    "5": (90, "M0 0H90V32H32V62H62A28 28 0 0 1 90 90V132A28 28 0 0 1 62 160H0V128H58V94H0Z"),
    "6": (90, "M28 0H90V32H32V62H62A28 28 0 0 1 90 90V132A28 28 0 0 1 62 160H28A28 28 0 0 1 0 132V28A28 28 0 0 1 28 0Z"
              "M32 94V128H58V94Z"),
    "/": (64, "M40 0H64L24 160H0Z"),
    "·": (32, "M0 64H32V96H0Z"),
})


def write(name, s):
    (OUT / name).write_text(s, encoding="utf-8")


def caixa(v, s):
    d = -v[0]
    x0, x1 = -d * s, 120 * s
    y0, y1 = -d * s, (160 + d) * s
    return x0, y0, x1 - x0, y1 - y0


def centrado(v, s, cy=126, face=VERDE, fill_n=BRANCO):
    x0, y0, w, h = caixa(v, s)
    return relevo(v=v, face=face, dx=128 - w / 2 - x0, dy=cy - h / 2 - y0, s=s, fill_n=fill_n)


def chapado(cor, s=1.15):
    """N sem relevo, para uma cor (carimbo, bordado, gravação)."""
    return f'<polygon points="{base.pts(N, 128 - 60 * s, 126 - 80 * s, s)}" fill="{cor}"/>'


def palavra(texto, x, cores, gaps=None, escala=1.0, y=0.0):
    gaps = gaps or {"e": 12, "x": 10, "t": 18, "C": 12, "/": 6, "·": 14, " ": 0}
    out = []
    for ch in texto:
        if ch == " ":
            x += 34 * escala
            continue
        w, d = G[ch]
        out.append(f'<path transform="translate({x:.2f} {y:.2f}) scale({escala})" d="{d}" fill="{cores(ch)}"/>')
        x += (w + gaps.get(ch, 10)) * escala
    return x, "".join(out)


# --- Símbolo ----------------------------------------------------------------
write("simbolo-escuro.svg", svg("0 0 256 256", centrado(V, 1.05), "NextCM — símbolo (fundo escuro)"))
write("simbolo-claro.svg", svg("0 0 256 256", centrado(V, 1.05, fill_n=ROXO), "NextCM — símbolo (fundo claro)"))
write("simbolo-pequeno.svg", svg("0 0 256 256", centrado(V_PEQ, 1.08), "NextCM — símbolo (16–48 px)"))
for nome, cor in (("preto", "#000000"), ("branco", BRANCO), ("roxo", ROXO)):
    write(f"simbolo-{nome}.svg", svg("0 0 256 256", chapado(cor), f"NextCM — símbolo numa cor ({nome})"))

# --- Ícones -----------------------------------------------------------------
TILE = f'<rect width="256" height="256" rx="56" fill="{ROXO}"/>'
write("icone.svg", svg("0 0 256 256", TILE + centrado(V, 0.92), "NextCM — ícone"))
write("icone-pequeno.svg", svg("0 0 256 256", TILE + centrado(V_PEQ, 1.0), "NextCM — ícone (16–48 px)"))
write("favicon.svg", svg("0 0 256 256", f'<rect width="256" height="256" rx="48" fill="{ROXO}"/>'
                         + centrado(V_PEQ, 1.06), "NextCM"))


# --- Horizontal (o N faz parte do nome) ---------------------------------------
def horizontal(fill_n, cor_txt, cor_cm, fundo=None, v=V, pad=30):
    vx, vy = v
    x_end, letras = palavra("extCM", 120 + 24, lambda c: cor_cm if c in "CM" else cor_txt)
    x0, y0 = vx - pad, vy - pad
    W, H = x_end - x0 + pad, 160 - vy + 2 * pad
    bg = f'<rect x="{x0}" y="{y0}" width="{W:.0f}" height="{H}" rx="24" fill="{fundo}"/>' if fundo else ""
    return f"{x0} {y0} {W:.0f} {H}", bg + relevo(v=v, face=VERDE, fill_n=fill_n) + letras


vb, b = horizontal(BRANCO, BRANCO, VERDE)
write("horizontal-escuro.svg", svg(vb, b, "NextCM — horizontal (fundo escuro)"))
vb, b = horizontal(ROXO, ESCURO, ROXO)
write("horizontal-claro.svg", svg(vb, b, "NextCM — horizontal (fundo claro)"))
for nome, cor in (("preto", "#000000"), ("branco", BRANCO)):
    x_end, letras = palavra("extCM", 144, lambda c, cor=cor: cor)
    corpo = f'<polygon points="{base.pts(N)}" fill="{cor}"/>' + letras
    write(f"horizontal-{nome}.svg", svg(f"-10 -10 {x_end + 20:.0f} 180", corpo, f"NextCM — horizontal numa cor ({nome})"))

# --- Tela inicial: maior, relevo mais fundo, a data a verde por baixo ----------
VT = (-20, -20)
x_end, letras = palavra("extCM", 144, lambda c: VERDE if c in "CM" else BRANCO)
_, data = palavra("01/02 · 25/26", 0, lambda c: VERDE, escala=0.22)
data_w = _
tela = (relevo(v=VT, face=VERDE, fill_n=BRANCO) + letras
        + f'<rect x="0" y="196" width="{x_end - data_w - 24:.0f}" height="8" fill="{VERDE}"/>'
        + f'<g transform="translate({x_end - data_w:.1f} 182)">{data}</g>')
vb_t = f"-60 -60 {x_end + 100:.0f} 310"
write("tela-inicial.svg", svg(vb_t, tela, "NextCM — tela inicial"))
write("tela-inicial-com-fundo.svg", svg(vb_t, f'<rect x="-60" y="-60" width="{x_end + 100:.0f}" height="310" rx="28" fill="{ESCURO}"/>'
                                        + tela, "NextCM — tela inicial (com fundo)"))
print("kit ok")
