//! Synchronous database access. Only the storage actor thread owns a [`Db`].

use std::path::Path;
use std::sync::LazyLock;

use ns_core::{CategoryKind, LocalDay, ProcessInfo, SegmentKind, Timestamp};
use rusqlite::{Connection, OptionalExtension, params};
use rusqlite_migration::{M, Migrations};
use serde::{Deserialize, Serialize};

use crate::StorageError;

static MIGRATIONS: LazyLock<Migrations<'static>> =
    LazyLock::new(|| Migrations::new(vec![M::up(include_str!("../migrations/001_init.sql"))]));

pub type Result<T> = std::result::Result<T, StorageError>;

pub struct Db {
    pub(crate) conn: Connection,
}

/// An app row as the tracker and the API see it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppRow {
    pub id: i64,
    pub exe_name: String,
    pub exe_path: Option<String>,
    pub display_name: Option<String>,
    pub category_id: i64,
    pub category: String,
    pub category_kind: CategoryKind,
    /// `auto` (categorizer) or `user` (set by hand, never overwritten).
    pub category_source: String,
    pub is_game: bool,
    pub first_seen: Timestamp,
    pub last_seen: Timestamp,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub kind: CategoryKind,
    pub color: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NagLogEntry {
    pub id: i64,
    pub rule_id: String,
    pub kind: String,
    pub level: Option<String>,
    pub reason: Option<String>,
    pub at: Timestamp,
}

const APP_COLUMNS: &str = "a.id, a.exe_name, a.exe_path, a.display_name, a.category_id, c.name, \
     c.kind, a.category_source, a.is_game, a.first_seen, a.last_seen";

fn app_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<AppRow> {
    let kind: String = r.get(6)?;
    Ok(AppRow {
        id: r.get(0)?,
        exe_name: r.get(1)?,
        exe_path: r.get(2)?,
        display_name: r.get(3)?,
        category_id: r.get(4)?,
        category: r.get(5)?,
        category_kind: kind.parse().unwrap_or_default(),
        category_source: r.get(7)?,
        is_game: r.get(8)?,
        first_seen: r.get(9)?,
        last_seen: r.get(10)?,
    })
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| StorageError::Io(e.to_string()))?;
        }
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        MIGRATIONS.to_latest(&mut conn)?;
        Ok(Self { conn })
    }

    // ---- apps and categories ----

    /// Finds the app by exe name or creates it with the given default
    /// category. Existing apps keep their category; only path and last_seen
    /// are refreshed.
    pub fn get_or_create_app(
        &mut self,
        process: &ProcessInfo,
        default_category: CategoryKind,
        is_game: bool,
        now: Timestamp,
    ) -> Result<AppRow> {
        let tx = self.conn.transaction()?;
        let existing: Option<i64> = tx
            .query_row(
                "SELECT id FROM apps WHERE exe_name = ?1",
                [&process.exe_name],
                |r| r.get(0),
            )
            .optional()?;
        let id = match existing {
            Some(id) => {
                tx.execute(
                    "UPDATE apps SET last_seen = ?2, exe_path = COALESCE(?3, exe_path),
                         package_id = COALESCE(?4, package_id) WHERE id = ?1",
                    params![id, now, process.exe_path, process.package_id],
                )?;
                id
            }
            None => {
                tx.execute(
                    "INSERT INTO apps (exe_name, exe_path, package_id, category_id, is_game,
                         first_seen, last_seen)
                     VALUES (?1, ?2, ?3, (SELECT id FROM categories WHERE kind = ?4 ORDER BY id LIMIT 1),
                         ?5, ?6, ?6)",
                    params![
                        process.exe_name,
                        process.exe_path,
                        process.package_id,
                        default_category.as_str(),
                        is_game,
                        now
                    ],
                )?;
                tx.last_insert_rowid()
            }
        };
        tx.commit()?;
        self.app(id)?.ok_or(StorageError::NotFound("app"))
    }

    pub fn app(&self, id: i64) -> Result<Option<AppRow>> {
        Ok(self
            .conn
            .query_row(
                &format!(
                    "SELECT {APP_COLUMNS} FROM apps a JOIN categories c ON c.id = a.category_id
                     WHERE a.id = ?1"
                ),
                [id],
                app_from_row,
            )
            .optional()?)
    }

    pub fn apps(&self) -> Result<Vec<AppRow>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {APP_COLUMNS} FROM apps a JOIN categories c ON c.id = a.category_id
             ORDER BY a.last_seen DESC"
        ))?;
        let rows = stmt.query_map([], app_from_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// User edit of an app; marks the category as user-chosen.
    pub fn update_app(
        &mut self,
        id: i64,
        category_id: Option<i64>,
        is_game: Option<bool>,
        display_name: Option<Option<String>>,
    ) -> Result<AppRow> {
        let tx = self.conn.transaction()?;
        if let Some(cat) = category_id {
            tx.execute(
                "UPDATE apps SET category_id = ?2, category_source = 'user' WHERE id = ?1",
                params![id, cat],
            )?;
        }
        if let Some(game) = is_game {
            tx.execute(
                "UPDATE apps SET is_game = ?2, category_source = 'user' WHERE id = ?1",
                params![id, game],
            )?;
        }
        if let Some(name) = display_name {
            tx.execute(
                "UPDATE apps SET display_name = ?2 WHERE id = ?1",
                params![id, name],
            )?;
        }
        tx.commit()?;
        self.app(id)?.ok_or(StorageError::NotFound("app"))
    }

    /// Sets a display name unless the user (or an earlier lookup) set one.
    pub fn set_display_name_if_empty(&mut self, id: i64, name: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE apps SET display_name = ?2 WHERE id = ?1 AND display_name IS NULL",
            params![id, name],
        )?;
        Ok(())
    }

    pub fn categories(&self) -> Result<Vec<Category>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, kind, color FROM categories ORDER BY id")?;
        let rows = stmt.query_map([], |r| {
            let kind: String = r.get(2)?;
            Ok(Category {
                id: r.get(0)?,
                name: r.get(1)?,
                kind: kind.parse().unwrap_or_default(),
                color: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn create_category(
        &mut self,
        name: &str,
        kind: CategoryKind,
        color: Option<&str>,
    ) -> Result<Category> {
        self.conn.execute(
            "INSERT INTO categories (name, kind, color) VALUES (?1, ?2, ?3)",
            params![name, kind.as_str(), color],
        )?;
        Ok(Category {
            id: self.conn.last_insert_rowid(),
            name: name.to_string(),
            kind,
            color: color.map(str::to_string),
        })
    }

    // ---- segments ----

    pub fn title_id(&mut self, text: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO titles (text) VALUES (?1) ON CONFLICT (text) DO NOTHING",
            [text],
        )?;
        Ok(self
            .conn
            .query_row("SELECT id FROM titles WHERE text = ?1", [text], |r| {
                r.get(0)
            })?)
    }

    /// Inserts a zero-length segment; grow it with [`Db::extend_segment`].
    pub fn open_segment(
        &mut self,
        app_id: Option<i64>,
        title_id: Option<i64>,
        kind: SegmentKind,
        start: Timestamp,
        local_day: LocalDay,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO segments (app_id, title_id, kind, start_ts, end_ts, local_day)
             VALUES (?1, ?2, ?3, ?4, ?4, ?5)",
            params![
                app_id,
                title_id,
                kind.as_str(),
                start,
                local_day.to_string()
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Moves a segment's end forward and adds the growth to the daily totals.
    /// Ends earlier than the current end are ignored.
    pub fn extend_segment(&mut self, id: i64, end: Timestamp) -> Result<()> {
        let tx = self.conn.transaction()?;
        let row: Option<(Option<i64>, String, i64, String)> = tx
            .query_row(
                "SELECT app_id, kind, end_ts, local_day FROM segments WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?;
        let Some((app_id, kind, old_end, day)) = row else {
            return Err(StorageError::NotFound("segment"));
        };
        let delta = end - old_end;
        if delta <= 0 {
            return Ok(());
        }
        tx.execute(
            "UPDATE segments SET end_ts = ?2 WHERE id = ?1",
            params![id, end],
        )?;
        tx.execute(
            "INSERT INTO daily_kind_totals (local_day, kind, ms) VALUES (?1, ?2, ?3)
             ON CONFLICT (local_day, kind) DO UPDATE SET ms = ms + excluded.ms",
            params![day, kind, delta],
        )?;
        if let (Some(app_id), "active") = (app_id, kind.as_str()) {
            tx.execute(
                "INSERT INTO daily_app_totals (local_day, app_id, ms) VALUES (?1, ?2, ?3)
                 ON CONFLICT (local_day, app_id) DO UPDATE SET ms = ms + excluded.ms",
                params![day, app_id, delta],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    // ---- web ----

    #[allow(clippy::too_many_arguments)]
    pub fn open_visit(
        &mut self,
        browser: &str,
        host: &str,
        url: Option<&str>,
        title: Option<&str>,
        tab_id: Option<i64>,
        start: Timestamp,
        local_day: LocalDay,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO web_visits (browser, host, url, title, tab_id, start_ts, end_ts, local_day)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?7)",
            params![
                browser,
                host,
                url,
                title,
                tab_id,
                start,
                local_day.to_string()
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn extend_visit(&mut self, id: i64, end: Timestamp) -> Result<()> {
        let tx = self.conn.transaction()?;
        let row: Option<(String, i64, String)> = tx
            .query_row(
                "SELECT host, end_ts, local_day FROM web_visits WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let Some((host, old_end, day)) = row else {
            return Err(StorageError::NotFound("web visit"));
        };
        let delta = end - old_end;
        if delta > 0 {
            tx.execute(
                "UPDATE web_visits SET end_ts = ?2 WHERE id = ?1",
                params![id, end],
            )?;
            tx.execute(
                "INSERT INTO daily_web_totals (local_day, host, ms) VALUES (?1, ?2, ?3)
                 ON CONFLICT (local_day, host) DO UPDATE SET ms = ms + excluded.ms",
                params![day, host, delta],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Records an extension event key; false if it was already seen.
    pub fn mark_ext_event(&mut self, client_id: &str, ts: Timestamp, tab_id: i64) -> Result<bool> {
        let n = self.conn.execute(
            "INSERT OR IGNORE INTO ext_events_seen (client_id, ts, tab_id) VALUES (?1, ?2, ?3)",
            params![client_id, ts, tab_id],
        )?;
        Ok(n == 1)
    }

    // ---- nags, rule state, settings ----

    pub fn log_nag(
        &mut self,
        rule_id: &str,
        kind: &str,
        level: Option<&str>,
        reason: Option<&str>,
        at: Timestamp,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO nag_events (rule_id, kind, level, reason, at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![rule_id, kind, level, reason, at],
        )?;
        Ok(())
    }

    pub fn nags(&self, since: Timestamp, limit: u32) -> Result<Vec<NagLogEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, rule_id, kind, level, reason, at FROM nag_events
             WHERE at >= ?1 ORDER BY at DESC, id DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![since, limit], |r| {
            Ok(NagLogEntry {
                id: r.get(0)?,
                rule_id: r.get(1)?,
                kind: r.get(2)?,
                level: r.get(3)?,
                reason: r.get(4)?,
                at: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn rule_states(&self) -> Result<Vec<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT rule_id, state_json FROM rule_state")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn save_rule_states(&mut self, states: &[(String, String)], now: Timestamp) -> Result<()> {
        let tx = self.conn.transaction()?;
        for (id, json) in states {
            tx.execute(
                "INSERT INTO rule_state (rule_id, state_json, updated_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT (rule_id) DO UPDATE SET state_json = excluded.state_json,
                     updated_at = excluded.updated_at",
                params![id, json, now],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn kv_get(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
                r.get(0)
            })
            .optional()?)
    }

    pub fn kv_set(&mut self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Deletes segments, visits and nag history older than `before_day`.
    /// Daily totals stay. Returns the number of deleted segments.
    pub fn prune(&mut self, before_day: LocalDay, before_ts: Timestamp) -> Result<usize> {
        let day = before_day.to_string();
        let tx = self.conn.transaction()?;
        let n = tx.execute("DELETE FROM segments WHERE local_day < ?1", [&day])?;
        tx.execute("DELETE FROM web_visits WHERE local_day < ?1", [&day])?;
        tx.execute("DELETE FROM ext_events_seen WHERE ts < ?1", [before_ts])?;
        tx.execute("DELETE FROM nag_events WHERE at < ?1", [before_ts])?;
        tx.execute(
            "DELETE FROM titles WHERE id NOT IN (SELECT title_id FROM segments WHERE title_id IS NOT NULL)",
            [],
        )?;
        tx.commit()?;
        Ok(n)
    }
}
