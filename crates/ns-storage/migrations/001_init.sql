-- Timestamps are unix milliseconds, UTC. local_day is YYYY-MM-DD after the
-- configured day start.

CREATE TABLE categories (
    id    INTEGER PRIMARY KEY,
    name  TEXT NOT NULL UNIQUE COLLATE NOCASE,
    kind  TEXT NOT NULL CHECK (kind IN ('Work', 'Game', 'Web', 'Other')),
    color TEXT
);

INSERT INTO categories (name, kind, color) VALUES
    ('Work', 'Work', '#3b82f6'),
    ('Game', 'Game', '#ef4444'),
    ('Web', 'Web', '#10b981'),
    ('Other', 'Other', '#9ca3af');

CREATE TABLE apps (
    id              INTEGER PRIMARY KEY,
    exe_name        TEXT NOT NULL UNIQUE COLLATE NOCASE,
    exe_path        TEXT,
    package_id      TEXT,
    display_name    TEXT,
    category_id     INTEGER NOT NULL REFERENCES categories (id),
    category_source TEXT NOT NULL DEFAULT 'auto' CHECK (category_source IN ('auto', 'user')),
    is_game         INTEGER NOT NULL DEFAULT 0,
    first_seen      INTEGER NOT NULL,
    last_seen       INTEGER NOT NULL
);

CREATE TABLE titles (
    id   INTEGER PRIMARY KEY,
    text TEXT NOT NULL UNIQUE
);

CREATE TABLE segments (
    id        INTEGER PRIMARY KEY,
    app_id    INTEGER REFERENCES apps (id),
    title_id  INTEGER REFERENCES titles (id),
    kind      TEXT NOT NULL CHECK (kind IN ('active', 'idle', 'locked', 'suspended', 'paused')),
    start_ts  INTEGER NOT NULL,
    end_ts    INTEGER NOT NULL,
    local_day TEXT NOT NULL
);
CREATE INDEX segments_day_app ON segments (local_day, app_id);
CREATE INDEX segments_start ON segments (start_ts);

CREATE TABLE web_visits (
    id        INTEGER PRIMARY KEY,
    browser   TEXT NOT NULL,
    host      TEXT NOT NULL,
    url       TEXT,
    title     TEXT,
    tab_id    INTEGER,
    start_ts  INTEGER NOT NULL,
    end_ts    INTEGER NOT NULL,
    local_day TEXT NOT NULL
);
CREATE INDEX web_visits_day_host ON web_visits (local_day, host);

-- Dedup keys for extension events so retried batches are idempotent.
CREATE TABLE ext_events_seen (
    client_id TEXT NOT NULL,
    ts        INTEGER NOT NULL,
    tab_id    INTEGER NOT NULL,
    PRIMARY KEY (client_id, ts, tab_id)
) WITHOUT ROWID;

-- Active time per app and day, kept up to date as segments grow so range
-- queries never scan segments.
CREATE TABLE daily_app_totals (
    local_day TEXT NOT NULL,
    app_id    INTEGER NOT NULL REFERENCES apps (id),
    ms        INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (local_day, app_id)
) WITHOUT ROWID;

CREATE TABLE daily_kind_totals (
    local_day TEXT NOT NULL,
    kind      TEXT NOT NULL,
    ms        INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (local_day, kind)
) WITHOUT ROWID;

CREATE TABLE daily_web_totals (
    local_day TEXT NOT NULL,
    host      TEXT NOT NULL,
    ms        INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (local_day, host)
) WITHOUT ROWID;

CREATE TABLE nag_events (
    id      INTEGER PRIMARY KEY,
    rule_id TEXT NOT NULL,
    kind    TEXT NOT NULL,
    level   TEXT,
    reason  TEXT,
    at      INTEGER NOT NULL
);
CREATE INDEX nag_events_at ON nag_events (at);

-- Engine counters per rule, so a restart does not reset a daily limit.
CREATE TABLE rule_state (
    rule_id    TEXT PRIMARY KEY,
    state_json TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Runtime key/value settings (API token, pairing state, ...).
CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
