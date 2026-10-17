"""
NextCM - Checagens do world.sqlite gerado pelo extract.py

Uso:
    python scripts/validate.py [caminho/world.sqlite]

1. Integridade: referências que apontam para IDs inexistentes.
2. Amostras: imprime registros conhecidos para conferir a olho com o editor original.
"""

import os
import sqlite3
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DB = sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "data", "world.sqlite")

# (tabela, coluna, tabela alvo)
REFERENCES = [
    ("people", "club_id", "clubs"),
    ("people", "nation_id", "nations"),
    ("people", "second_nation_id", "nations"),
    ("people", "national_team_id", "clubs"),
    ("people", "club_job", "jobs"),
    ("clubs", "nation_id", "nations"),
    ("clubs", "division_id", "competitions"),
    ("clubs", "reserve_division_id", "competitions"),
    ("clubs", "stadium_id", "stadiums"),
    ("stadiums", "city_id", "cities"),
    ("nations", "continent_id", "continents"),
    ("competitions", "nation_id", "nations"),
    ("competitions", "continent_id", "continents"),
    ("competition_history", "competition_id", "competitions"),
    ("competition_history", "winner_id", "clubs"),
    ("career_history", "person_id", "people"),
    ("career_history", "club_id", "clubs"),
]


def main():
    if not os.path.exists(DB):
        sys.exit(f"[ERRO] {DB} não existe. Rode scripts/extract.py antes.")
    db = sqlite3.connect(DB)
    q = lambda sql, *a: db.execute(sql, a).fetchall()

    print("== Integridade")
    problems = 0
    for table, col, target in REFERENCES:
        n = q(f"SELECT COUNT(*) FROM {table} t WHERE t.{col} IS NOT NULL "
              f"AND NOT EXISTS (SELECT 1 FROM {target} x WHERE x.id = t.{col})")[0][0]
        status = "ok" if n == 0 else f"{n} órfãos"
        problems += n
        print(f"  {table}.{col} -> {target}: {status}")

    print("\n== Jogadores conhecidos")
    for name in ("Lionel Messi", "Erling Haaland", "Giorgian de Arrascaeta", "Agustín Rossi"):
        rows = q("""SELECT p.display_name, p.birth_date, n.name, c.name, j.name, p.contract_expires,
                           pl.current_ability, pl.potential_ability, pl.finishing, pl.handling,
                           pl.left_foot, pl.right_foot
                    FROM people p
                    JOIN players pl ON pl.person_id = p.id
                    LEFT JOIN nations n ON n.id = p.nation_id
                    LEFT JOIN clubs c ON c.id = p.club_id
                    LEFT JOIN jobs j ON j.id = p.club_job
                    WHERE p.first_name || ' ' || p.second_name = ?""", name)
        for r in rows:
            print(f"  {r[0]:24} nasc={r[1]} {r[2]:10} {r[3]:32} {r[4]:8} contrato={r[5]} "
                  f"CA/PA={r[6]}/{r[7]} fin={r[8]} gk_hand={r[9]} pés E/D={r[10]}/{r[11]}")

    print("\n== Flamengo")
    club = q("SELECT c.id, c.name, n.name, d.name, e.name, s.name, s.capacity, c.reputation "
             "FROM clubs c JOIN nations n ON n.id = c.nation_id "
             "LEFT JOIN competitions d ON d.id = c.division_id "
             "LEFT JOIN competitions e ON e.id = c.reserve_division_id "
             "LEFT JOIN stadiums s ON s.id = c.stadium_id "
             "WHERE c.short_name = 'Flamengo' AND n.name = 'Brazil'")[0]
    print(f"  {club[1]} | {club[2]} | {club[3]} | estadual: {club[4]} | {club[5]} ({club[6]}) | rep {club[7]}")
    for job, n in q("SELECT j.name, COUNT(*) FROM people p JOIN jobs j ON j.id = p.club_job "
                    "WHERE p.club_id = ? GROUP BY j.name ORDER BY COUNT(*) DESC", club[0]):
        print(f"    {job}: {n}")
    coach = q("SELECT display_name FROM people WHERE club_id = ? AND club_job = 5", club[0])
    print(f"    Treinador: {', '.join(r[0] for r in coach)}")

    print("\n== Série A (clubes na divisão)")
    rows = q("SELECT c.short_name FROM clubs c JOIN competitions d ON d.id = c.division_id "
             "WHERE d.name = 'Campeonato Brasileiro Série A' ORDER BY c.reputation DESC")
    print(f"  {len(rows)} clubes: {', '.join(r[0] for r in rows)}")

    print("\n== Campeões recentes da Libertadores")
    for y, w, ru in q("SELECT h.year, w.short_name, r.short_name FROM competition_history h "
                      "JOIN competitions c ON c.id = h.competition_id "
                      "LEFT JOIN clubs w ON w.id = h.winner_id LEFT JOIN clubs r ON r.id = h.runner_up_id "
                      "WHERE c.name = 'Copa Libertadores de América' ORDER BY h.year DESC LIMIT 5"):
        print(f"  {y}: {w} (vice {ru})")

    print("\n== Carreira do Messi")
    for y, c, a, g in q("SELECT h.year, c.short_name, h.apps, h.goals FROM career_history h "
                        "JOIN people p ON p.id = h.person_id LEFT JOIN clubs c ON c.id = h.club_id "
                        "WHERE p.first_name = 'Lionel' AND p.second_name = 'Messi' ORDER BY h.year DESC LIMIT 6"):
        print(f"  {y} {c:22} {a:3} jogos {g:3} gols")

    print(f"\n{'[OK]' if problems == 0 else '[ATENÇÃO]'} {problems} referências órfãs no total")


if __name__ == "__main__":
    main()
