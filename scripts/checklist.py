"""
NextCM - Ficha para conferir o world.sqlite contra o editor original (cm0102ed-nick).

Uso:
    python scripts/checklist.py > docs/conferencia-editor.md

Mostra os valores *crus* do banco (os mesmos que o editor exibe, atributos 1–20),
com os nomes de campo em inglês, como aparecem no editor.
"""

import os
import sqlite3
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DB = sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "data", "world.sqlite")

PLAYERS = ["Lionel Messi", "Erling Haaland", "Giorgian de Arrascaeta", "Lamine Yamal", "Raphael Veiga"]
STAFF = ["Pep Guardiola", "Carlo Ancelotti", "Filipe Luís"]
CLUBS = ["Clube de Regatas do Flamengo", "Sociedade Esportiva Palmeiras", "Real Madrid C.F."]
COMPS = ["Campeonato Brasileiro Série A", "Copa Libertadores de América"]

MENTAL = ["adaptability", "ambition", "determination", "loyalty", "pressure", "professionalism",
          "sportsmanship", "temperament"]
POSITIONS = ["goalkeeper", "sweeper", "defender", "defensive_midfielder", "midfielder",
             "attacking_midfielder", "attacker", "wing_back", "right_side", "left_side", "central", "free_role"]
# Ordem em que o extrator lê os 21 atributos de staff (ainda não validada)
STAFF_ATTRS = ["attacking", "business", "coaching", "coaching_gks", "coaching_technique", "directness",
               "discipline", "free_roles", "interference", "judgement", "judging_potential", "man_handling",
               "marking", "motivating", "offside", "patience", "physiotherapy", "pressing", "resources",
               "tactics", "youngsters"]


def label(col):
    return col.replace("_", " ").title()


def table(rows, cols=3):
    """Pares (campo, valor) em `cols` colunas de Markdown, com uma coluna vazia para o valor do editor."""
    out = ["| " + " | ".join(["Campo", "Banco", "Editor"] * cols) + " |",
           "|" + "---|---:|---|" * cols]
    per = -(-len(rows) // cols)
    for i in range(per):
        cells = []
        for c in range(cols):
            k = c * per + i
            cells += [rows[k][0], str(rows[k][1]), " "] if k < len(rows) else ["", "", ""]
        out.append("| " + " | ".join(cells) + " |")
    return "\n".join(out)


def main():
    db = sqlite3.connect(DB)
    db.row_factory = sqlite3.Row

    def one(sql, *a):
        return db.execute(sql, a).fetchone()

    player_cols = [r[1] for r in db.execute("PRAGMA table_info(players)")]
    attr_cols = player_cols[player_cols.index("acceleration"):player_cols.index("morale")]

    print("# Conferência com o editor (cm0102ed-nick)\n")
    print("Abra o editor com os `.dat` de `data/raw/clz-25-26/` (a pasta `Data/` do patch instalado tem o banco "
          "de 2001). Para cada registro, preencha a coluna **Editor** só onde o valor for diferente. "
          "As datas já estão com +24 anos (no editor aparecem 24 anos antes: 2025 → 2001).\n")
    print("Gerado por `python scripts/checklist.py`.\n")

    print("## Jogadores\n")
    for name in PLAYERS:
        r = one("""SELECT p.*, n.name nation, c.name club, j.name job, pl.*
                   FROM people p JOIN players pl ON pl.person_id = p.id
                   LEFT JOIN nations n ON n.id = p.nation_id LEFT JOIN clubs c ON c.id = p.club_id
                   LEFT JOIN jobs j ON j.id = p.club_job
                   WHERE p.display_name = ? ORDER BY pl.current_ability DESC""", name)
        if r is None:
            print(f"### {name}\n\n_não encontrado_\n")
            continue
        print(f"### {name} (id {r['id']})\n")
        base = [("Born", r["birth_date"]), ("Nation", r["nation"]), ("Club", r["club"]), ("Job", r["job"]),
                ("Squad No", r["squad_number"]), ("Contract Ends", r["contract_expires"]), ("Wage", r["wage"]),
                ("Int. Apps/Goals", f"{r['international_apps']}/{r['international_goals']}"),
                ("Current Ability", r["current_ability"]), ("Potential Ability", r["potential_ability"]),
                ("Home/Cur/World Rep", f"{r['home_reputation']}/{r['current_reputation']}/{r['world_reputation']}"),
                ("Morale", r["morale"])]
        print(table(base) + "\n")
        print(table([(label(p), r["pos_" + p]) for p in POSITIONS]) + "\n")
        print(table([(label(a), r[a]) for a in attr_cols] + [(label(m), r[m]) for m in MENTAL]) + "\n")

    print("## Staff (ordem dos 21 atributos)\n")
    print("Se os valores aparecerem trocados entre campos (ex.: o nosso *Coaching* bate com o *Tactics* do editor), "
          "anote na coluna Editor o nome do campo onde o valor aparece.\n")
    for name in STAFF:
        r = one("""SELECT p.*, c.name club, j.name job, nt.name national_team, nj.name national_job, np.*
                   FROM people p JOIN non_players np ON np.person_id = p.id
                   LEFT JOIN clubs c ON c.id = p.club_id LEFT JOIN jobs j ON j.id = p.club_job
                   LEFT JOIN clubs nt ON nt.id = p.national_team_id LEFT JOIN jobs nj ON nj.id = p.national_team_job
                   WHERE p.display_name = ?""", name)
        if r is None:
            print(f"### {name}\n\n_não encontrado_\n")
            continue
        if r["club"]:
            where = f"{r['job']} do {r['club']}"
        elif r["national_team"]:
            where = f"{r['national_job']} da seleção ({r['national_team']})"
        else:
            where = "sem clube"
        print(f"### {name} (id {r['id']}): {where}\n")
        base = [("Current Ability", r["current_ability"]), ("Potential Ability", r["potential_ability"]),
                ("Home/Cur/World Rep", f"{r['home_reputation']}/{r['current_reputation']}/{r['world_reputation']}"),
                ("Preferred Formation", r["preferred_formation"])]
        print(table(base, 2) + "\n")
        print(table([(label(a), r[a]) for a in STAFF_ATTRS] + [(label(m), r[m]) for m in MENTAL]) + "\n")

    print("## Clubes\n")
    for name in CLUBS:
        r = one("""SELECT c.*, n.name nation, d.name division, rd.name reserve, s.name stadium, s.capacity
                   FROM clubs c LEFT JOIN nations n ON n.id = c.nation_id
                   LEFT JOIN competitions d ON d.id = c.division_id
                   LEFT JOIN competitions rd ON rd.id = c.reserve_division_id
                   LEFT JOIN stadiums s ON s.id = c.stadium_id WHERE c.name = ?""", name)
        if r is None:
            print(f"### {name}\n\n_não encontrado_\n")
            continue
        rivals = [one("SELECT name FROM clubs WHERE id = ?", r[k]) for k in ("rival1_id", "rival2_id", "rival3_id")]
        squad = one("SELECT COUNT(*) n FROM people WHERE club_id = ? AND is_player = 1", r["id"])["n"]
        print(f"### {name} (id {r['id']})\n")
        rows = [("Short Name", r["short_name"]), ("Nation", r["nation"]), ("Division", r["division"]),
                ("Reserve Division", r["reserve"]), ("Last Position", r["last_position"]),
                ("Reputation", r["reputation"]), ("Cash", r["cash"]), ("Status", r["professional_status"]),
                ("Stadium", f"{r['stadium']} ({r['capacity']})"),
                ("Rivals", ", ".join(x["name"] for x in rivals if x)), ("Jogadores no plantel", squad)]
        print(table(rows, 2) + "\n")

    print("## Competições\n")
    for name in COMPS:
        r = one("""SELECT cp.*, n.name nation, ct.name continent FROM competitions cp
                   LEFT JOIN nations n ON n.id = cp.nation_id LEFT JOIN continents ct ON ct.id = cp.continent_id
                   WHERE cp.name = ?""", name)
        if r is None:
            print(f"### {name}\n\n_não encontrada_\n")
            continue
        n_clubs = one("SELECT COUNT(*) n FROM clubs WHERE division_id = ?", r["id"])["n"]
        last = db.execute("""SELECT h.year, w.name FROM competition_history h LEFT JOIN clubs w ON w.id = h.winner_id
                             WHERE h.competition_id = ? ORDER BY h.year DESC LIMIT 3""", (r["id"],)).fetchall()
        print(f"### {name} (id {r['id']})\n")
        rows = [("Short Name", r["short_name"]), ("Abbreviation", r["abbreviation"]), ("Nation", r["nation"]),
                ("Continent", r["continent"]), ("Reputation", r["reputation"]), ("Clubes na divisão", n_clubs),
                ("Últimos campeões", "; ".join(f"{y} {w}" for y, w in last))]
        print(table(rows, 2) + "\n")


if __name__ == "__main__":
    main()
