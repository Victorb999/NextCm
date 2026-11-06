"""
NextCM - O que o CM aplica ao começar um jogo, a partir do player_setup.cfg do patch.

O CM 01/02 lê o `player_setup.cfg` (escrito pelo CM0102ED, na pasta Data do jogo) ao começar
um jogo; o banco (.dat) não tem nada disto. Este script cruza as linhas com `data/world.sqlite`
(pessoa pelo nome, clubes pelo nome completo) e grava dois ficheiros que o core lê ao criar o
jogo:

  - `core/data/setup_loans.json`: "LOAN" (jogador, dono, clube do empréstimo, fim).
  - `core/data/setup_events.json`: "INJURY" (lesionado ao começar, com a duração do tipo),
    "FUTURE_TRANSFER" (mudança acertada para uma data, com valor, salário e anos), "RETIREMENT"
    (reforma numa data), os castigos "MISCONDUCT", "FALSE_PASSPORT" e "DRUGS" (sem jogar até
    uma data) e "INT_RETIREMENT" (retirado da selecção: nunca mais é convocado).

Ficam de fora "MANAGER_CONF" e "PRESET_TACTICS".

Uso:
    python scripts/player_setup.py
    python scripts/player_setup.py --cfg <player_setup.cfg> --year-offset 24
"""

import argparse
import datetime as dt
import json
import os
import shlex
import sqlite3
import sys
import unicodedata

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEFAULT_CFG = os.path.join(ROOT, "data", "raw", "clz-25-26", "player_setup.cfg")
DEFAULT_DB = os.path.join(ROOT, "data", "world.sqlite")
LOANS_OUT = os.path.join(ROOT, "core", "data", "setup_loans.json")
EVENTS_OUT = os.path.join(ROOT, "core", "data", "setup_events.json")

# As lesões do CM: o nome no jogo e os dias de paragem (mínimo e máximo)
INJURIES = {
    "DAMAGED_CRUCIATE_LIG": ("Lesão nos ligamentos cruzados", 150, 240),
    "TORN_KNEE_LIG": ("Rotura de ligamentos do joelho", 60, 150),
    "BROKEN_FOOT": ("Fractura no pé", 35, 70),
    "TORN_HAMSTRING": ("Rotura nos isquiotibiais", 21, 56),
    "TORN_ANKLE_LIG": ("Rotura de ligamentos do tornozelo", 28, 70),
    "SHIN_SPLINTS": ("Canelite", 10, 28),
    "TORN_GROIN_MUSCLE": ("Rotura muscular na virilha", 21, 42),
    "DAMAGED_ACHILLES_TEN": ("Lesão no tendão de Aquiles", 60, 180),
    "BROKEN_SHOULDER": ("Fractura no ombro", 42, 84),
    "TORN_CALF_MUSCLE": ("Rotura muscular na barriga da perna", 21, 42),
    "COLD": ("Constipação", 3, 7),
    "DAMAGED_KNEE_CAP": ("Lesão na rótula", 30, 90),
    "BROKEN_ANKLE": ("Fractura no tornozelo", 60, 120),
    "BROKEN_COLLARBONE": ("Fractura da clavícula", 42, 70),
    "BROKEN_LEG_1": ("Fractura da perna", 90, 180),
    "BROKEN_LEG_2": ("Fractura da perna", 90, 180),
    "SERIOUS_VIRAL_INFECTION": ("Infecção viral grave", 14, 42),
    "DAMAGED_KNEE_CART": ("Lesão na cartilagem do joelho", 60, 150),
    "BROKEN_TOE": ("Fractura num dedo do pé", 14, 35),
    "DAMAGED_SHOULDER": ("Lesão no ombro", 21, 56),
    "DAMAGED_FOOT": ("Lesão no pé", 14, 42),
    "BROKEN_RIBS": ("Fractura de costelas", 21, 42),
    "SPRAINED_ANKLE": ("Entorse no tornozelo", 7, 21),
    "HIP_INJURY": ("Lesão na anca", 14, 42),
    "CALF_STRAIN": ("Distensão na barriga da perna", 7, 21),
    "FRACTURED_WRIST": ("Fractura no pulso", 28, 56),
    "BROKEN_WRIST": ("Fractura no pulso", 28, 56),
    "STRAINED_ANKLE_LIG": ("Distensão de ligamentos do tornozelo", 10, 28),
    "DISLOCATED_JAW": ("Luxação do maxilar", 14, 35),
    "PULLED_HAMSTRING": ("Distensão nos isquiotibiais", 7, 21),
    "TWISTED_ANKLE": ("Entorse no tornozelo", 5, 14),
    "BACK_STRAIN": ("Dores nas costas", 5, 14),
    "BROKEN_CHEEKBONE": ("Fractura da maçã do rosto", 21, 42),
    "FRACTURED_CHEEKBONE": ("Fractura da maçã do rosto", 21, 42),
    "BRUISED_THIGH": ("Contusão na coxa", 3, 10),
    "BROKEN_HAND": ("Fractura na mão", 21, 42),
    "BROKEN_PELVIS": ("Fractura da bacia", 90, 150),
    "ARTHRITIS": ("Artrite", 30, 90),
    "FACIAL_INJURY": ("Lesão no rosto", 7, 21),
    "DAMAGED_ELBOW": ("Lesão no cotovelo", 14, 42),
}
# Os castigos: a chave gravada e o que o jogo diz
BANS = {"MISCONDUCT": "misconduct", "FALSE_PASSPORT": "passport", "DRUGS": "drugs"}


def fold(s):
    s = unicodedata.normalize("NFKD", s or "")
    return "".join(c for c in s if not unicodedata.combining(c)).lower().strip()


def date(d, m, y, offset):
    """Dia, mês e ano do patch (o jogo começa em 2001) -> "AAAA-MM-DD"."""
    try:
        return dt.date(int(y) + offset, int(m), int(d)).isoformat()
    except ValueError:
        return None


class Base:
    def __init__(self, path):
        con = sqlite3.connect(path)
        # Clubes pelo nome completo (num nome repetido ficam todos; decide o clube do jogador)
        self.clubs = {}
        for cid, name in con.execute("SELECT id, name FROM clubs WHERE is_national_team = 0"):
            self.clubs.setdefault(fold(name), []).append(cid)
        # Pessoas pelo nome e apelido (e pela alcunha, quando a linha a tem)
        self.people = {}
        for pid, first, second, common, club in con.execute(
                "SELECT id, first_name, second_name, common_name, club_id FROM people WHERE is_player = 1"):
            self.people.setdefault((fold(first), fold(second)), []).append((pid, fold(common), club))

    def club(self, name):
        return self.clubs.get(fold(name), [])

    def person(self, first, common, second, clubs=None):
        """A pessoa da linha: no clube dado (se houver), senão a única com esse nome."""
        found = [p for p in self.people.get((fold(first), fold(second)), []) if not common or p[1] == fold(common)]
        if clubs is not None:
            found = [p for p in found if p[2] in clubs]
        return found[0] if len(found) == 1 else None


def write(path, rows):
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write("[\n" + ",\n".join(json.dumps(r, ensure_ascii=False, separators=(", ", ": ")) for r in rows) + "\n]\n")


def main():
    ap = argparse.ArgumentParser(description="player_setup.cfg -> core/data/setup_loans.json e setup_events.json")
    ap.add_argument("--cfg", default=DEFAULT_CFG)
    ap.add_argument("--db", default=DEFAULT_DB)
    ap.add_argument("--year-offset", type=int, default=24)
    a = ap.parse_args()
    base = Base(a.db)

    loans, events, missing = [], [], []
    with open(a.cfg, encoding="latin1") as f:
        for line in f:
            if not line.startswith('"'):
                continue
            try:
                x = shlex.split(line)
            except ValueError:
                missing.append(line.strip())
                continue
            kind = x[0]
            if kind == "LOAN" and len(x) == 12:
                _, first, common, second, parent, club, _d1, _m1, _y1, d2, m2, y2 = x
                parents, loan_clubs = base.club(parent), base.club(club)
                p = base.person(first, common, second, parents + loan_clubs)
                until = date(d2, m2, y2, a.year_offset)
                if not p or not parents or not loan_clubs or until is None:
                    missing.append(line.strip())
                    continue
                owner = p[2] if p[2] in parents else parents[0]
                to = p[2] if p[2] in loan_clubs else loan_clubs[0]
                if owner != to:
                    loans.append({"player": p[0], "parent": owner, "club": to, "until": until})
            elif kind == "INJURY" and len(x) == 6:
                _, first, common, second, club, injury = x
                p = base.person(first, common, second, base.club(club) or None)
                name, low, high = INJURIES.get(injury, ("Lesão", 14, 42))
                if not p:
                    missing.append(line.strip())
                    continue
                events.append({"kind": "injury", "player": p[0], "injury": name, "min": low, "max": high})
            elif kind == "FUTURE_TRANSFER" and len(x) == 12:
                _, first, common, second, club, to, d, m, y, fee, wage, years = x
                froms, tos = base.club(club), base.club(to)
                p = base.person(first, common, second, froms + tos)
                day = date(d, m, y, a.year_offset)
                if not p or not tos or day is None:
                    missing.append(line.strip())
                    continue
                events.append({"kind": "transfer", "player": p[0], "from": p[2], "to": tos[0], "date": day,
                               "fee": int(fee), "wage": int(wage), "years": max(1, int(years))})
            elif kind == "INT_RETIREMENT" and len(x) == 5:
                _, first, common, second, club = x
                p = base.person(first, common, second, base.club(club) or None) or base.person(first, common, second)
                if not p:
                    missing.append(line.strip())
                    continue
                # O patch repete algumas linhas
                if {"kind": "intl_retirement", "player": p[0]} not in events:
                    events.append({"kind": "intl_retirement", "player": p[0]})
            elif (kind == "RETIREMENT" or kind in BANS) and len(x) == 8:
                _, first, common, second, club, d, m, y = x
                p = base.person(first, common, second, base.club(club) or None)
                day = date(d, m, y, a.year_offset)
                if not p or day is None:
                    missing.append(line.strip())
                    continue
                if kind == "RETIREMENT":
                    events.append({"kind": "retirement", "player": p[0], "date": day})
                else:
                    events.append({"kind": "ban", "player": p[0], "ban": BANS[kind], "date": day})

    loans.sort(key=lambda r: r["player"])
    events.sort(key=lambda r: (r["kind"], r["player"]))
    write(LOANS_OUT, loans)
    write(EVENTS_OUT, events)
    counts = {}
    for e in events:
        counts[e["kind"]] = counts.get(e["kind"], 0) + 1
    print(f"{len(loans)} empréstimos e {counts} gravados; {len(missing)} linhas sem par na base")
    for line in missing[:10]:
        print("  sem par:", line)
    return 0


if __name__ == "__main__":
    sys.exit(main())
