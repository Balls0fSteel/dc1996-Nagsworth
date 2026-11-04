"""Fills a Nagsworth database with plausible demo data for UI checks.

Usage: python seed_demo.py <path to activity.db> [days=14]

The app must have created the database (run it once with
NAGSWORTH_DATA_DIR pointing at the scratch folder) and must be stopped
while seeding: the segmenter updates its open segment by row id, and a
deleted id would be reused by a seeded row. Existing demo rows are
replaced: apps are matched by exe name, segments and totals for the seeded
days are deleted first.
"""

import json
import random
import sqlite3
import sys
from datetime import datetime, timedelta

DAY_START_H = 4

APPS = [
    # exe, display name, category, is_game, weight, titles
    ("Code.exe", "Visual Studio Code", "Work", 0, 5, ["nagsworth - main.rs", "ui - App.svelte", "PLAN.md"]),
    ("WindowsTerminal.exe", "Windows Terminal", "Work", 0, 2, ["cargo test", "PowerShell"]),
    ("msedge.exe", "Microsoft Edge", "Web", 0, 3, ["GitHub", "docs.rs", "YouTube"]),
    ("firefox.exe", None, "Web", 0, 1, ["Mozilla Firefox"]),
    ("Discord.exe", "Discord", "Other", 0, 1, ["Discord"]),
    ("explorer.exe", None, "Other", 0, 1, ["Downloads"]),
    ("steam.exe", "Steam", "Game", 0, 1, ["Steam"]),
    ("eldenring.exe", "ELDEN RING", "Game", 1, 2, ["ELDEN RING"]),
    ("factorio.exe", "Factorio", "Game", 1, 2, ["Factorio 2.0"]),
    ("obsidian.exe", "Obsidian", "Work", 0, 1, ["notes - Obsidian"]),
]

HOSTS = ["github.com", "docs.rs", "youtube.com", "news.ycombinator.com", "reddit.com",
         "stackoverflow.com", "crates.io", "wikipedia.org"]

# Routines as the engine logs them (config/exercises.toml ids).
ROUTINES = {
    "rest-break": [
        ("walk", "Walk", 180, "1-5 min"),
        ("neck-iso", "Neck presses", 45, "5 s each direction, x 3"),
        ("hip-flexor-stretch", "Hip flexor stretch", 60, "30 s each side"),
        ("far-focus", "Look far away", 20, "20 s"),
        ("cyclic-sigh", "Cyclic sighing", 45, "5 breaths"),
    ],
    "move-break": [("walk", "Walk", 64, "1-5 min"), ("squats", "Bodyweight squats", 45, "10-15")],
    "micro-break": [("blink-squeeze", "Full blinks", 30, "10 slow")],
}


def routine_json(rule: str) -> str | None:
    steps = ROUTINES.get(rule)
    if not steps:
        return None
    return json.dumps({
        "steps": [
            {"id": i, "name": n, "seconds": s, "reps": r, "cue": f"{n}.", "lines": [f"{n}."]}
            for i, n, s, r in steps
        ],
        "note": "1 h 20 min without a real break: walk first." if rule == "rest-break" else None,
    })


def local_day(dt: datetime) -> str:
    return (dt - timedelta(hours=DAY_START_H)).date().isoformat()


def ms(dt: datetime) -> int:
    return int(dt.timestamp() * 1000)


def main() -> None:
    path = sys.argv[1]
    days = int(sys.argv[2]) if len(sys.argv) > 2 else 14
    rng = random.Random(7)
    con = sqlite3.connect(path)
    cur = con.cursor()
    cats = {name: cid for cid, name in cur.execute("SELECT id, name FROM categories")}
    now = datetime.now().astimezone()
    today = local_day(now)
    first = datetime.fromisoformat(today) - timedelta(days=days - 1)

    app_ids = {}
    for exe, disp, cat, game, _w, _t in APPS:
        row = cur.execute("SELECT id FROM apps WHERE exe_name = ? COLLATE NOCASE", (exe,)).fetchone()
        if row:
            app_ids[exe] = row[0]
        else:
            cur.execute(
                "INSERT INTO apps (exe_name, exe_path, display_name, category_id, category_source,"
                " is_game, first_seen, last_seen) VALUES (?, ?, ?, ?, 'auto', ?, ?, ?)",
                (exe, f"C:\\Program Files\\{exe}", disp, cats[cat], game, ms(first), ms(now)),
            )
            app_ids[exe] = cur.lastrowid

    title_ids = {}

    def title_id(text: str) -> int:
        if text not in title_ids:
            row = cur.execute("SELECT id FROM titles WHERE text = ?", (text,)).fetchone()
            if row:
                title_ids[text] = row[0]
            else:
                cur.execute("INSERT INTO titles (text) VALUES (?)", (text,))
                title_ids[text] = cur.lastrowid
        return title_ids[text]

    seeded_days = [(first + timedelta(days=i)).date().isoformat() for i in range(days)]
    q = ",".join("?" * len(seeded_days))
    for table in ("segments", "daily_app_totals", "daily_kind_totals", "daily_web_totals", "web_visits"):
        cur.execute(f"DELETE FROM {table} WHERE local_day IN ({q})", seeded_days)
    cur.execute("DELETE FROM nag_events")

    weights = [a[4] for a in APPS]
    for day in seeded_days:
        d0 = datetime.fromisoformat(day).replace(tzinfo=now.tzinfo)
        weekend = d0.weekday() >= 5
        t = d0 + timedelta(hours=9 if not weekend else 11, minutes=rng.randint(0, 50))
        end_of_day = d0 + timedelta(hours=22 if not weekend else 24)
        if day == today:
            end_of_day = min(end_of_day, now)
        app_tot: dict[int, int] = {}
        app_input: dict[int, int] = {}
        kind_input: dict[str, int] = {}
        kind_tot: dict[str, int] = {}
        web_tot: dict[str, int] = {}
        while t < end_of_day:
            r = rng.random()
            if r < 0.08:
                kind, length = "idle", rng.randint(3, 25) * 60_000
            elif r < 0.11:
                kind, length = "locked", rng.randint(20, 70) * 60_000
            else:
                kind, length = "active", rng.randint(2, 40) * 60_000
            if weekend or t.hour >= 19:
                # Evenings and weekends lean toward games.
                w = [wt * (3 if a[2] == "Game" else 1) for a, wt in zip(APPS, weights)]
            else:
                w = [wt * (0.15 if a[2] == "Game" else 1) for a, wt in zip(APPS, weights)]
            app = rng.choices(APPS, weights=w)[0]
            t_end = min(t + timedelta(milliseconds=length), end_of_day)
            if t_end <= t:
                break
            length = ms(t_end) - ms(t)
            if kind == "active":
                aid = app_ids[app[0]]
                # Hands-on time: games have cutscenes and chat, work is mostly typing.
                share = rng.uniform(0.45, 0.75) if app[2] == "Game" else rng.uniform(0.8, 0.98)
                input_ms = int(length * share)
                cur.execute(
                    "INSERT INTO segments (app_id, title_id, kind, start_ts, end_ts, local_day, input_ms)"
                    " VALUES (?, ?, 'active', ?, ?, ?, ?)",
                    (aid, title_id(rng.choice(app[5])), ms(t), ms(t_end), day, input_ms),
                )
                app_tot[aid] = app_tot.get(aid, 0) + length
                app_input[aid] = app_input.get(aid, 0) + input_ms
                kind_input["active"] = kind_input.get("active", 0) + input_ms
                if app[2] == "Web":
                    host = rng.choice(HOSTS)
                    cur.execute(
                        "INSERT INTO web_visits (browser, host, url, title, tab_id, start_ts, end_ts, local_day)"
                        " VALUES ('edge', ?, ?, ?, 1, ?, ?, ?)",
                        (host, f"https://{host}/", host, ms(t), ms(t_end), day),
                    )
                    web_tot[host] = web_tot.get(host, 0) + length
            else:
                cur.execute(
                    "INSERT INTO segments (app_id, title_id, kind, start_ts, end_ts, local_day)"
                    " VALUES (NULL, NULL, ?, ?, ?, ?)",
                    (kind, ms(t), ms(t_end), day),
                )
            kind_tot[kind] = kind_tot.get(kind, 0) + length
            t = t_end
        for aid, total in app_tot.items():
            cur.execute(
                "INSERT INTO daily_app_totals (local_day, app_id, ms, input_ms) VALUES (?, ?, ?, ?)",
                (day, aid, total, app_input.get(aid, 0)),
            )
        for kind, total in kind_tot.items():
            cur.execute(
                "INSERT INTO daily_kind_totals (local_day, kind, ms, input_ms) VALUES (?, ?, ?, ?)",
                (day, kind, total, kind_input.get(kind, 0)),
            )
        for host, total in web_tot.items():
            cur.execute("INSERT INTO daily_web_totals (local_day, host, ms) VALUES (?, ?, ?)", (day, host, total))

    # A few nag events over the last days.
    for i in range(16):
        at = now - timedelta(hours=rng.randint(1, days * 20))
        rule = rng.choice(["rest-break", "move-break", "micro-break", "gaming-daily"])
        kind = rng.choice(["raise", "escalate", "dismiss", "snooze", "skip", "break_started"])
        level = rng.choice(["toast", "overlay"]) if kind in ("raise", "escalate") else None
        reason = {"dismiss": "rested", "snooze": "user", "skip": "user"}.get(kind)
        routine = routine_json(rule) if kind in ("raise", "break_started") else None
        cur.execute(
            "INSERT INTO nag_events (rule_id, kind, level, reason, at, routine) VALUES (?, ?, ?, ?, ?, ?)",
            (rule, kind, level, reason, ms(at), routine),
        )
    # Started breaks with their end, for the History "Breaks" card: most run
    # to the end, some are ended early. Each pair is inserted together, as
    # the breaks query pairs a start with the next dismiss by row order.
    for i in range(14):
        rule = rng.choice(["rest-break", "move-break", "micro-break"])
        length = {"rest-break": 600, "move-break": 120, "micro-break": 30}[rule]
        start = now - timedelta(hours=rng.randint(1, days * 20))
        early = rng.random() < 0.25
        end = start + timedelta(seconds=rng.randint(5, length - 1) if early else length)
        cur.execute(
            "INSERT INTO nag_events (rule_id, kind, level, reason, at, routine) VALUES (?, 'break_started', NULL, NULL, ?, ?)",
            (rule, ms(start), routine_json(rule)),
        )
        cur.execute(
            "INSERT INTO nag_events (rule_id, kind, level, reason, at, routine) VALUES (?, 'dismiss', NULL, ?, ?, NULL)",
            (rule, "ended_early" if early else "break_taken", ms(end)),
        )
    con.commit()
    print(f"seeded {days} days into {path}")


if __name__ == "__main__":
    main()
