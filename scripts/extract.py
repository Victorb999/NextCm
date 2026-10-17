"""
NextCM - Extrator do banco do CM 01/02 -> world.sqlite

Uso:
    python scripts/extract.py                       # usa data/raw/clz-25-26 -> data/world.sqlite
    python scripts/extract.py --src <pasta Data> --out <arquivo.sqlite> --year-offset 0

Layout dos arquivos (todos little-endian):
  - index.dat: header de 8 bytes + entradas de 67 bytes (nome[51], id, quantidade, offset, versão).
    É a fonte da verdade para contagens e offsets de cada tabela.
  - staff.dat: várias tabelas em sequência (pessoas, non-players, players, preferências).
  - Strings: char[N] terminadas em \\0, codificação Windows-1252.
  - Datas (TDate, 8 bytes): short dia_do_ano (base 0), short ano, int ano_bissexto.
    Ano 1900 = data desconhecida.

O patch CLZ 25-26 guarda todos os anos com -24 (o jogo começa em 2001, que equivale a 2025).
Por isso --year-offset vale 24 por padrão; para o banco original do CM 01/02 use 0.
"""

import argparse
import datetime as dt
import os
import sqlite3
import struct
import sys
import time
import unicodedata

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEFAULT_SRC = os.path.join(ROOT, "data", "raw", "clz-25-26")
DEFAULT_OUT = os.path.join(ROOT, "data", "world.sqlite")

# IDs das tabelas no index.dat
T_CLUB, T_NAT_CLUB, T_CONTINENT, T_NATION, T_STADIUM = 0, 1, 3, 4, 5
T_STAFF, T_NONPLAYER, T_PLAYER = 6, 9, 10
T_CLUB_COMP, T_FIRST_NAMES, T_SECOND_NAMES, T_COMMON_NAMES = 12, 13, 14, 15
T_NATION_COMP, T_STAFF_HISTORY, T_CLUB_COMP_HISTORY, T_NATION_COMP_HISTORY = 16, 17, 19, 20
T_CITY = 21

JOBS = {
    1: "Presidente",
    2: "Diretor Executivo",
    3: "Diretor de Futebol",
    4: "Diretor",
    5: "Treinador",
    6: "Auxiliar Técnico",
    7: "Treinador do Time B",
    8: "Preparador",
    9: "Olheiro",
    10: "Fisioterapeuta",
    11: "Jogador",
    12: "Jogador/Treinador",
    13: "Jogador/Preparador",
    14: "Jogador/Fisioterapeuta",
    15: "Treinador Interino",
}

POSITIONS = [
    "goalkeeper", "sweeper", "defender", "defensive_midfielder", "midfielder",
    "attacking_midfielder", "attacker", "wing_back",
    "right_side", "left_side", "central", "free_role",
]

PLAYER_ATTRIBUTES = [
    "acceleration", "aggression", "agility", "anticipation", "balance", "bravery",
    "consistency", "corners", "crossing", "decisions", "dirtiness", "dribbling",
    "finishing", "flair", "free_kicks", "handling", "heading", "important_matches",
    "injury_proneness", "jumping", "leadership", "left_foot", "long_shots", "marking",
    "off_the_ball", "natural_fitness", "one_on_ones", "pace", "passing", "penalties",
    "positioning", "reflexes", "right_foot", "stamina", "strength", "tackling",
    "teamwork", "technique", "throw_ins", "versatility", "vision", "work_rate",
]

# Ordem ainda não validada contra o editor original (ver ROADMAP).
NONPLAYER_ATTRIBUTES = [
    "attacking", "business", "coaching", "coaching_gks", "coaching_technique",
    "directness", "discipline", "free_roles", "interference", "judgement",
    "judging_potential", "man_handling", "marking", "motivating", "offside",
    "patience", "physiotherapy", "pressing", "resources", "tactics", "youngsters",
]

# Bits de `people.positions` (posições naturais, valor >= 15). Espelhado em core/src/search.rs.
NATURAL_POSITION = 15
POSITION_BITS = {
    "goalkeeper": 1, "sweeper": 2, "defender": 2, "wing_back": 4,
    "defensive_midfielder": 8, "midfielder": 16, "attacking_midfielder": 32, "attacker": 64,
    "right_side": 128, "left_side": 256, "central": 512,
}

MENTAL = [
    "adaptability", "ambition", "determination", "loyalty",
    "pressure", "professionalism", "sportsmanship", "temperament",
]


# ---------------------------------------------------------------- leitura binária

def i32(b, o):
    return struct.unpack_from("<i", b, o)[0]


def i16(b, o):
    return struct.unpack_from("<h", b, o)[0]


def u8(b, o):
    return b[o]


def text(b, o, size):
    return b[o:o + size].split(b"\0", 1)[0].decode("cp1252", errors="replace").strip()


def ref(v):
    """IDs negativos (-1, -2) significam 'nenhum'."""
    return v if v >= 0 else None


def fold(s):
    """Minúsculas sem acento, para busca: 'Paquetá' -> 'paqueta'."""
    s = unicodedata.normalize("NFKD", s)
    return "".join(c for c in s if not unicodedata.combining(c)).lower()


def nz(v):
    """Atributo 0 = não definido no banco (o jogo gera na hora)."""
    return v if v > 0 else None


class Reader:
    def __init__(self, src, year_offset):
        self.src = src
        self.year_offset = year_offset
        self.index = self._read_index()
        self._cache = {}

    def _read_index(self):
        data = self._file("index.dat")
        index = {}
        for o in range(8, len(data) - 66, 67):
            name = text(data, o, 51)
            tid, count, offset, version = struct.unpack_from("<iiii", data, o + 51)
            index[tid] = (name, count, offset, version)
        return index

    def _file(self, name):
        path = os.path.join(self.src, name)
        if not os.path.exists(path):
            sys.exit(f"[ERRO] Arquivo não encontrado: {path}")
        with open(path, "rb") as f:
            return f.read()

    def table(self, tid, record_size):
        """Itera (índice, bytes) dos registros da tabela `tid`, validando o tamanho."""
        name, count, offset, _ = self.index[tid]
        if name not in self._cache:
            self._cache[name] = self._file(name)
        data = self._cache[name]
        end = offset + count * record_size
        if end > len(data):
            sys.exit(f"[ERRO] {name} (tabela {tid}): {count} x {record_size} bytes passa do fim do arquivo")
        for k in range(count):
            o = offset + k * record_size
            yield k, data[o:o + record_size]

    def date(self, b, o):
        day, year, _leap = struct.unpack_from("<hhi", b, o)
        if year <= 1900:
            return None
        year += self.year_offset
        try:
            return (dt.date(year, 1, 1) + dt.timedelta(days=day)).isoformat()
        except (ValueError, OverflowError):
            return None

    def year(self, y):
        return y + self.year_offset if y > 0 else None


def staff_record_size(r):
    """O tamanho do registro de pessoa varia entre patches; deduz pelo offset da próxima tabela."""
    _, count, _, _ = r.index[T_STAFF]
    return r.index[T_NONPLAYER][2] // count


# ---------------------------------------------------------------- schema

SCHEMA = f"""
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT);

CREATE TABLE continents (
    id INTEGER PRIMARY KEY, name TEXT, federation TEXT, abbreviation TEXT
);

CREATE TABLE nations (
    id INTEGER PRIMARY KEY, name TEXT, short_name TEXT, code TEXT, nationality TEXT,
    continent_id INTEGER, capital_city_id INTEGER, stadium_id INTEGER
);

CREATE TABLE cities (id INTEGER PRIMARY KEY, name TEXT);

CREATE TABLE stadiums (
    id INTEGER PRIMARY KEY, name TEXT, city_id INTEGER,
    capacity INTEGER, seating_capacity INTEGER, expansion_capacity INTEGER
);

CREATE TABLE competitions (
    id INTEGER PRIMARY KEY,
    kind TEXT NOT NULL,              -- 'club' | 'national'
    name TEXT, short_name TEXT, abbreviation TEXT,
    continent_id INTEGER, nation_id INTEGER, reputation INTEGER
);

CREATE TABLE competition_history (
    competition_id INTEGER, year INTEGER,
    winner_id INTEGER, runner_up_id INTEGER, third_id INTEGER
);

CREATE TABLE clubs (
    id INTEGER PRIMARY KEY,
    is_national_team INTEGER NOT NULL,
    name TEXT, short_name TEXT, nation_id INTEGER,
    division_id INTEGER, last_division_id INTEGER, last_position INTEGER,
    reserve_division_id INTEGER,    -- no patch CLZ: campeonato estadual
    professional_status INTEGER, cash INTEGER, stadium_id INTEGER, reputation INTEGER,
    rival1_id INTEGER, rival2_id INTEGER, rival3_id INTEGER
);

CREATE TABLE jobs (id INTEGER PRIMARY KEY, name TEXT);

CREATE TABLE people (
    id INTEGER PRIMARY KEY,
    display_name TEXT NOT NULL, first_name TEXT, second_name TEXT, common_name TEXT,
    search_name TEXT NOT NULL,       -- nome comum + completo, minúsculo e sem acento
    list_name TEXT NOT NULL,         -- como nas listas do CM: "Sobrenome, Nome" ou o nome comum
    birth_date TEXT, birth_year INTEGER,
    nation_id INTEGER, second_nation_id INTEGER,
    international_apps INTEGER, international_goals INTEGER,
    national_team_id INTEGER, national_team_job INTEGER,
    club_id INTEGER, club_job INTEGER,
    club_joined TEXT, contract_expires TEXT, wage INTEGER,
    {", ".join(f"{a} INTEGER" for a in MENTAL)},
    is_player INTEGER NOT NULL, is_non_player INTEGER NOT NULL,
    -- Colunas derivadas só para ordenar/filtrar rápido na busca:
    ability INTEGER,                 -- CA do jogador, senão CA do staff
    birth_sort TEXT,                 -- birth_date, ou 'AAAA-07-01' quando só há o ano
    positions INTEGER NOT NULL       -- bitmask de POSITION_BITS
);

CREATE TABLE players (
    person_id INTEGER PRIMARY KEY,
    squad_number INTEGER,
    current_ability INTEGER, potential_ability INTEGER,
    home_reputation INTEGER, current_reputation INTEGER, world_reputation INTEGER,
    {", ".join(f"pos_{p} INTEGER" for p in POSITIONS)},
    {", ".join(f"{a} INTEGER" for a in PLAYER_ATTRIBUTES)},
    morale INTEGER
);

CREATE TABLE non_players (
    person_id INTEGER PRIMARY KEY,
    current_ability INTEGER, potential_ability INTEGER,
    home_reputation INTEGER, current_reputation INTEGER, world_reputation INTEGER,
    {", ".join(f"{a} INTEGER" for a in NONPLAYER_ATTRIBUTES)},
    preferred_formation INTEGER
);

CREATE TABLE career_history (
    person_id INTEGER, year INTEGER, club_id INTEGER,
    on_loan INTEGER, apps INTEGER, goals INTEGER
);
"""

INDEXES = """
CREATE INDEX idx_people_club ON people(club_id);
CREATE INDEX idx_people_nation ON people(nation_id);
CREATE INDEX idx_people_name ON people(display_name COLLATE NOCASE);
CREATE INDEX idx_people_list_name ON people(list_name COLLATE NOCASE);
CREATE INDEX idx_people_ability ON people(ability DESC, display_name);
CREATE INDEX idx_people_birth ON people(birth_sort DESC, display_name);
CREATE INDEX idx_clubs_nation ON clubs(nation_id);
CREATE INDEX idx_clubs_division ON clubs(division_id);
CREATE INDEX idx_clubs_reserve_division ON clubs(reserve_division_id);
CREATE INDEX idx_competitions_nation ON competitions(nation_id);
CREATE INDEX idx_history_person ON career_history(person_id);
CREATE INDEX idx_comp_history ON competition_history(competition_id, year);
"""


def insert(db, table, rows):
    rows = list(rows)
    if not rows:
        return 0
    cols = list(rows[0].keys())
    sql = f"INSERT INTO {table} ({', '.join(cols)}) VALUES ({', '.join('?' * len(cols))})"
    db.executemany(sql, ([row[c] for c in cols] for row in rows))
    return len(rows)


# ---------------------------------------------------------------- extratores

def names(r, tid):
    out = {}
    for k, b in r.table(tid, 60):
        out[i32(b, 51)] = text(b, 0, 51)
    return out


def continents(r):
    for _, b in r.table(T_CONTINENT, 198):
        yield {"id": i32(b, 0), "name": text(b, 4, 51),
               "federation": text(b, 61, 101), "abbreviation": text(b, 163, 26)}


def nations(r):
    for _, b in r.table(T_NATION, 290):
        yield {"id": i32(b, 0), "name": text(b, 4, 51), "short_name": text(b, 56, 26),
               "code": text(b, 83, 4), "nationality": text(b, 87, 26),
               "continent_id": ref(i32(b, 113)), "capital_city_id": ref(i32(b, 122)),
               "stadium_id": ref(i32(b, 128))}


def cities(r):
    for _, b in r.table(T_CITY, 56):
        yield {"id": i32(b, 0), "name": text(b, 4, 51)}


def stadiums(r):
    for _, b in r.table(T_STADIUM, 78):
        yield {"id": i32(b, 0), "name": text(b, 4, 51), "city_id": ref(i32(b, 56)),
               "capacity": i32(b, 60), "seating_capacity": i32(b, 64),
               "expansion_capacity": i32(b, 68)}


def competitions(r):
    for tid, kind in ((T_CLUB_COMP, "club"), (T_NATION_COMP, "national")):
        for _, b in r.table(tid, 107):
            yield {"id": i32(b, 0), "kind": kind, "name": text(b, 4, 51),
                   "short_name": text(b, 56, 26), "abbreviation": text(b, 83, 4),
                   "continent_id": ref(i32(b, 89)), "nation_id": ref(i32(b, 93)),
                   "reputation": i16(b, 105)}


def competition_history(r):
    for tid in (T_CLUB_COMP_HISTORY, T_NATION_COMP_HISTORY):
        for _, b in r.table(tid, 26):
            _, comp, year, winner, runner_up, third, _host = struct.unpack_from("<iihiiii", b, 0)
            yield {"competition_id": comp, "year": r.year(year), "winner_id": ref(winner),
                   "runner_up_id": ref(runner_up), "third_id": ref(third)}


def clubs(r):
    for tid, national in ((T_CLUB, 0), (T_NAT_CLUB, 1)):
        for _, b in r.table(tid, 581):
            yield {"id": i32(b, 0), "is_national_team": national,
                   "name": text(b, 4, 51), "short_name": text(b, 56, 26),
                   "nation_id": ref(i32(b, 83)), "division_id": ref(i32(b, 87)),
                   "last_division_id": ref(i32(b, 91)), "last_position": u8(b, 95) or None,
                   "reserve_division_id": ref(i32(b, 96)), "professional_status": u8(b, 100),
                   "cash": i32(b, 101), "stadium_id": ref(i32(b, 105)),
                   "reputation": i16(b, 128),
                   "rival1_id": ref(i32(b, 179)), "rival2_id": ref(i32(b, 183)),
                   "rival3_id": ref(i32(b, 187))}


def people(r, first, second, common):
    size = staff_record_size(r)
    players, non_players = {}, {}
    for k, b in r.table(T_PLAYER, 70):
        players[k] = b
    for k, b in r.table(T_NONPLAYER, 68):
        non_players[k] = b

    people_rows, player_rows, nonplayer_rows = [], [], []
    for _, b in r.table(T_STAFF, size):
        pid = i32(b, 0)
        fn, sn, cn = first.get(i32(b, 4), ""), second.get(i32(b, 8), ""), common.get(i32(b, 12), "")
        display = cn or f"{fn} {sn}".strip() or f"#{pid}"
        player_ptr, nonplayer_ptr = i32(b, 97), i32(b, 105)

        row = {"id": pid, "display_name": display,
               "first_name": fn or None, "second_name": sn or None, "common_name": cn or None,
               "search_name": fold(f"{cn} {fn} {sn}".strip()),
               "list_name": cn or (f"{sn}, {fn}" if sn and fn else display),
               "birth_date": r.date(b, 16), "birth_year": r.year(i16(b, 24)),
               "nation_id": ref(i32(b, 26)), "second_nation_id": ref(i32(b, 30)),
               "international_apps": u8(b, 34), "international_goals": u8(b, 35),
               "national_team_id": ref(i32(b, 36)), "national_team_job": u8(b, 40) or None,
               "club_id": ref(i32(b, 57)), "club_job": u8(b, 61) or None,
               "club_joined": r.date(b, 62), "contract_expires": r.date(b, 70),
               "wage": i32(b, 78),
               "is_player": int(player_ptr in players),
               "is_non_player": int(nonplayer_ptr in non_players)}
        if row["birth_year"] is None and row["birth_date"]:
            row["birth_year"] = int(row["birth_date"][:4])
        for n, attr in enumerate(MENTAL):
            row[attr] = nz(u8(b, 86 + n))
        row["birth_sort"] = row["birth_date"] or (f"{row['birth_year']}-07-01" if row["birth_year"] else None)
        row["ability"] = None
        row["positions"] = 0
        people_rows.append(row)

        if player_ptr in players:
            p = players[player_ptr]
            prow = {"person_id": pid, "squad_number": u8(p, 4) or None,
                    "current_ability": i16(p, 5), "potential_ability": i16(p, 7),
                    "home_reputation": i16(p, 9), "current_reputation": i16(p, 11),
                    "world_reputation": i16(p, 13)}
            for n, pos in enumerate(POSITIONS):
                prow[f"pos_{pos}"] = nz(u8(p, 15 + n))
            for n, attr in enumerate(PLAYER_ATTRIBUTES):
                prow[attr] = nz(u8(p, 27 + n))
            prow["morale"] = nz(u8(p, 69))
            player_rows.append(prow)
            row["ability"] = prow["current_ability"]
            for pos, bit in POSITION_BITS.items():
                if (prow[f"pos_{pos}"] or 0) >= NATURAL_POSITION:
                    row["positions"] |= bit

        if nonplayer_ptr in non_players:
            p = non_players[nonplayer_ptr]
            nrow = {"person_id": pid,
                    "current_ability": i16(p, 4), "potential_ability": i16(p, 6),
                    "home_reputation": i16(p, 8), "current_reputation": i16(p, 10),
                    "world_reputation": i16(p, 12)}
            for n, attr in enumerate(NONPLAYER_ATTRIBUTES):
                nrow[attr] = nz(u8(p, 14 + n))
            nrow["preferred_formation"] = u8(p, 67)
            nonplayer_rows.append(nrow)
            if row["ability"] is None:
                row["ability"] = nrow["current_ability"]

    return people_rows, player_rows, nonplayer_rows


def career_history(r):
    seen = set()  # o banco tem entradas duplicadas
    for _, b in r.table(T_STAFF_HISTORY, 17):
        _, person, year, club = struct.unpack_from("<iihi", b, 0)
        row = (person, r.year(year), ref(club), u8(b, 14), u8(b, 15), u8(b, 16))
        if row in seen:
            continue
        seen.add(row)
        yield dict(zip(("person_id", "year", "club_id", "on_loan", "apps", "goals"), row))


# ---------------------------------------------------------------- main

def main():
    ap = argparse.ArgumentParser(description="Converte os .dat do CM 01/02 em world.sqlite")
    ap.add_argument("--src", default=DEFAULT_SRC, help="pasta com os .dat (padrão: data/raw/clz-25-26)")
    ap.add_argument("--out", default=DEFAULT_OUT, help="arquivo de saída (padrão: data/world.sqlite)")
    ap.add_argument("--year-offset", type=int, default=24,
                    help="anos somados às datas do banco (24 no patch CLZ 25-26, 0 no original)")
    args = ap.parse_args()

    t0 = time.time()
    r = Reader(args.src, args.year_offset)
    print(f"[*] Lendo {args.src}")
    print(f"[*] Registro de pessoa: {staff_record_size(r)} bytes")

    os.makedirs(os.path.dirname(os.path.abspath(args.out)), exist_ok=True)
    tmp = args.out + ".tmp"
    if os.path.exists(tmp):
        os.remove(tmp)
    db = sqlite3.connect(tmp)
    db.executescript(SCHEMA)

    first, second, common = names(r, T_FIRST_NAMES), names(r, T_SECOND_NAMES), names(r, T_COMMON_NAMES)
    people_rows, player_rows, nonplayer_rows = people(r, first, second, common)

    counts = {
        "continents": insert(db, "continents", continents(r)),
        "nations": insert(db, "nations", nations(r)),
        "cities": insert(db, "cities", cities(r)),
        "stadiums": insert(db, "stadiums", stadiums(r)),
        "competitions": insert(db, "competitions", competitions(r)),
        "competition_history": insert(db, "competition_history", competition_history(r)),
        "clubs": insert(db, "clubs", clubs(r)),
        "jobs": insert(db, "jobs", ({"id": k, "name": v} for k, v in JOBS.items())),
        "people": insert(db, "people", people_rows),
        "players": insert(db, "players", player_rows),
        "non_players": insert(db, "non_players", nonplayer_rows),
        "career_history": insert(db, "career_history", career_history(r)),
    }
    db.executescript(INDEXES)
    db.execute("ANALYZE")  # estatísticas para o planejador escolher bons índices
    insert(db, "meta", ({"key": k, "value": str(v)} for k, v in {
        "source": os.path.basename(os.path.normpath(args.src)),
        "year_offset": args.year_offset,
        "extracted_at": dt.datetime.now().isoformat(timespec="seconds"),
        "schema_version": 6,
    }.items()))
    db.commit()
    db.close()
    os.replace(tmp, args.out)

    for k, v in counts.items():
        print(f"    {k:22} {v:8}")
    print(f"[OK] {args.out} gerado em {time.time() - t0:.1f}s")


if __name__ == "__main__":
    main()
