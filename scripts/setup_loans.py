"""
NextCM - Empréstimos do início do jogo a partir do player_setup.cfg do patch.

O CM 01/02 aplica o `player_setup.cfg` (escrito pelo CM0102ED, na pasta Data do jogo) ao
começar um jogo: as linhas "LOAN" põem cada jogador no clube do empréstimo, com o fim. O banco
(.dat) não os tem. Este script cruza cada linha com `data/world.sqlite` (pessoa pelo nome e
clubes pelo nome completo) e grava `core/data/setup_loans.json`, que o core lê ao criar o jogo.

Uso:
    python scripts/setup_loans.py                                  # Data do jogo -> core/data/setup_loans.json
    python scripts/setup_loans.py --cfg <player_setup.cfg> --year-offset 24
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
DEFAULT_OUT = os.path.join(ROOT, "core", "data", "setup_loans.json")


def fold(s):
    s = unicodedata.normalize("NFKD", s or "")
    return "".join(c for c in s if not unicodedata.combining(c)).lower().strip()


def date(d, m, y, offset):
    """Dia, mês e ano do patch (o jogo começa em 2001) -> "AAAA-MM-DD"."""
    try:
        return dt.date(int(y) + offset, int(m), int(d)).isoformat()
    except ValueError:
        return None


def main():
    ap = argparse.ArgumentParser(description="Empréstimos do player_setup.cfg -> core/data/setup_loans.json")
    ap.add_argument("--cfg", default=DEFAULT_CFG)
    ap.add_argument("--db", default=DEFAULT_DB)
    ap.add_argument("--out", default=DEFAULT_OUT)
    ap.add_argument("--year-offset", type=int, default=24)
    a = ap.parse_args()

    con = sqlite3.connect(a.db)
    # Clubes pelo nome completo (num nome repetido ficam todos; decide o clube do jogador)
    clubs = {}
    for cid, name in con.execute("SELECT id, name FROM clubs WHERE is_national_team = 0"):
        clubs.setdefault(fold(name), []).append(cid)
    # Pessoas pelo nome e apelido (e pela alcunha, quando a linha a tem)
    people = {}
    for pid, first, second, common, club in con.execute(
            "SELECT id, first_name, second_name, common_name, club_id FROM people WHERE is_player = 1"):
        people.setdefault((fold(first), fold(second)), []).append((pid, fold(common), club))

    out, missing = [], []
    with open(a.cfg, encoding="latin1") as f:
        for line in f:
            if not line.startswith('"LOAN"'):
                continue
            try:
                _, first, common, second, parent, club, d1, m1, y1, d2, m2, y2 = shlex.split(line)
            except ValueError:
                missing.append(line.strip())
                continue
            parents, loan_clubs = clubs.get(fold(parent), []), clubs.get(fold(club), [])
            # O jogador está no dono ou já no clube do empréstimo
            candidates = [p for p in people.get((fold(first), fold(second)), [])
                          if (not common or p[1] == fold(common)) and p[2] in parents + loan_clubs]
            if len(candidates) != 1 or not parents or not loan_clubs:
                missing.append(line.strip())
                continue
            pid, _, at = candidates[0]
            owner = at if at in parents else parents[0]
            to = at if at in loan_clubs else loan_clubs[0]
            until = date(d2, m2, y2, a.year_offset)
            if until is None or owner == to:
                missing.append(line.strip())
                continue
            out.append({"player": pid, "parent": owner, "club": to, "until": until})

    out.sort(key=lambda l: l["player"])
    with open(a.out, "w", encoding="utf-8", newline="\n") as f:
        f.write("[\n" + ",\n".join(json.dumps(l, separators=(", ", ": ")) for l in out) + "\n]\n")
    print(f"{len(out)} empréstimos gravados em {os.path.relpath(a.out, ROOT)}; {len(missing)} sem par na base")
    for line in missing[:15]:
        print("  sem par:", line)
    return 0


if __name__ == "__main__":
    sys.exit(main())
