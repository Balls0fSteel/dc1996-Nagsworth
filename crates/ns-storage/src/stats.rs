//! Read-only reports for the dashboard and API. Range queries read the daily
//! total tables; only the timeline reads segments.

use ns_core::{CategoryKind, LocalDay, SegmentKind, Timestamp};
use serde::{Deserialize, Serialize};

use crate::db::{Db, Result};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppUsage {
    pub app_id: i64,
    pub exe_name: String,
    pub display_name: Option<String>,
    pub category: String,
    pub category_kind: CategoryKind,
    pub is_game: bool,
    pub ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoryUsage {
    pub category_id: i64,
    pub name: String,
    pub kind: CategoryKind,
    pub color: Option<String>,
    pub ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailyUsage {
    pub day: LocalDay,
    pub category: String,
    pub kind: CategoryKind,
    pub color: Option<String>,
    pub ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KindTotal {
    pub day: LocalDay,
    pub kind: SegmentKind,
    pub ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub id: i64,
    pub start: Timestamp,
    pub end: Timestamp,
    pub kind: SegmentKind,
    pub app_id: Option<i64>,
    pub exe_name: Option<String>,
    pub title: Option<String>,
    pub category_kind: Option<CategoryKind>,
}

/// One segment with names resolved, for exports.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportSegment {
    pub day: LocalDay,
    pub start: Timestamp,
    pub end: Timestamp,
    pub kind: SegmentKind,
    pub exe_name: Option<String>,
    pub display_name: Option<String>,
    pub category: Option<String>,
    pub title: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportVisit {
    pub day: LocalDay,
    pub start: Timestamp,
    pub end: Timestamp,
    pub browser: String,
    pub host: String,
    pub url: Option<String>,
    pub title: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostUsage {
    pub host: String,
    pub ms: i64,
}

fn day(s: String) -> rusqlite::Result<LocalDay> {
    s.parse().map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })
}

impl Db {
    pub fn usage_by_app(&self, from: LocalDay, to: LocalDay) -> Result<Vec<AppUsage>> {
        let mut stmt = self.conn.prepare(
            "SELECT a.id, a.exe_name, a.display_name, c.name, c.kind, a.is_game, SUM(t.ms) AS ms
             FROM daily_app_totals t
             JOIN apps a ON a.id = t.app_id
             JOIN categories c ON c.id = a.category_id
             WHERE t.local_day BETWEEN ?1 AND ?2
             GROUP BY a.id ORDER BY ms DESC",
        )?;
        let rows = stmt.query_map([from.to_string(), to.to_string()], |r| {
            let kind: String = r.get(4)?;
            Ok(AppUsage {
                app_id: r.get(0)?,
                exe_name: r.get(1)?,
                display_name: r.get(2)?,
                category: r.get(3)?,
                category_kind: kind.parse().unwrap_or_default(),
                is_game: r.get(5)?,
                ms: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn usage_by_category(&self, from: LocalDay, to: LocalDay) -> Result<Vec<CategoryUsage>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.id, c.name, c.kind, c.color, SUM(t.ms) AS ms
             FROM daily_app_totals t
             JOIN apps a ON a.id = t.app_id
             JOIN categories c ON c.id = a.category_id
             WHERE t.local_day BETWEEN ?1 AND ?2
             GROUP BY c.id ORDER BY ms DESC",
        )?;
        let rows = stmt.query_map([from.to_string(), to.to_string()], |r| {
            let kind: String = r.get(2)?;
            Ok(CategoryUsage {
                category_id: r.get(0)?,
                name: r.get(1)?,
                kind: kind.parse().unwrap_or_default(),
                color: r.get(3)?,
                ms: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn daily_usage(&self, from: LocalDay, to: LocalDay) -> Result<Vec<DailyUsage>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.local_day, c.name, c.kind, c.color, SUM(t.ms)
             FROM daily_app_totals t
             JOIN apps a ON a.id = t.app_id
             JOIN categories c ON c.id = a.category_id
             WHERE t.local_day BETWEEN ?1 AND ?2
             GROUP BY t.local_day, c.id ORDER BY t.local_day, c.id",
        )?;
        let rows = stmt.query_map([from.to_string(), to.to_string()], |r| {
            let kind: String = r.get(2)?;
            Ok(DailyUsage {
                day: day(r.get(0)?)?,
                category: r.get(1)?,
                kind: kind.parse().unwrap_or_default(),
                color: r.get(3)?,
                ms: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn kind_totals(&self, from: LocalDay, to: LocalDay) -> Result<Vec<KindTotal>> {
        let mut stmt = self.conn.prepare(
            "SELECT local_day, kind, ms FROM daily_kind_totals
             WHERE local_day BETWEEN ?1 AND ?2 ORDER BY local_day, kind",
        )?;
        let rows = stmt.query_map([from.to_string(), to.to_string()], |r| {
            let kind: String = r.get(1)?;
            Ok((day(r.get(0)?)?, kind, r.get(2)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (day, kind, ms) = row?;
            if let Ok(kind) = kind.parse() {
                out.push(KindTotal { day, kind, ms });
            }
        }
        Ok(out)
    }

    pub fn timeline(&self, day: LocalDay) -> Result<Vec<TimelineEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.id, s.start_ts, s.end_ts, s.kind, s.app_id, a.exe_name, t.text, c.kind
             FROM segments s
             LEFT JOIN apps a ON a.id = s.app_id
             LEFT JOIN categories c ON c.id = a.category_id
             LEFT JOIN titles t ON t.id = s.title_id
             WHERE s.local_day = ?1 ORDER BY s.start_ts",
        )?;
        let rows = stmt.query_map([day.to_string()], |r| {
            let kind: String = r.get(3)?;
            let cat: Option<String> = r.get(7)?;
            Ok(TimelineEntry {
                id: r.get(0)?,
                start: r.get(1)?,
                end: r.get(2)?,
                kind: kind.parse().unwrap_or(SegmentKind::Active),
                app_id: r.get(4)?,
                exe_name: r.get(5)?,
                title: r.get(6)?,
                category_kind: cat.and_then(|c| c.parse().ok()),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn usage_by_host(&self, from: LocalDay, to: LocalDay) -> Result<Vec<HostUsage>> {
        let mut stmt = self.conn.prepare(
            "SELECT host, SUM(ms) AS ms FROM daily_web_totals
             WHERE local_day BETWEEN ?1 AND ?2 GROUP BY host ORDER BY ms DESC",
        )?;
        let rows = stmt.query_map([from.to_string(), to.to_string()], |r| {
            Ok(HostUsage {
                host: r.get(0)?,
                ms: r.get(1)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn export_segments(&self, from: LocalDay, to: LocalDay) -> Result<Vec<ExportSegment>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.local_day, s.start_ts, s.end_ts, s.kind, a.exe_name, a.display_name, c.name, t.text
             FROM segments s
             LEFT JOIN apps a ON a.id = s.app_id
             LEFT JOIN categories c ON c.id = a.category_id
             LEFT JOIN titles t ON t.id = s.title_id
             WHERE s.local_day BETWEEN ?1 AND ?2 ORDER BY s.start_ts",
        )?;
        let rows = stmt.query_map([from.to_string(), to.to_string()], |r| {
            let kind: String = r.get(3)?;
            Ok(ExportSegment {
                day: day(r.get(0)?)?,
                start: r.get(1)?,
                end: r.get(2)?,
                kind: kind.parse().unwrap_or(SegmentKind::Active),
                exe_name: r.get(4)?,
                display_name: r.get(5)?,
                category: r.get(6)?,
                title: r.get(7)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn export_visits(&self, from: LocalDay, to: LocalDay) -> Result<Vec<ExportVisit>> {
        let mut stmt = self.conn.prepare(
            "SELECT local_day, start_ts, end_ts, browser, host, url, title FROM web_visits
             WHERE local_day BETWEEN ?1 AND ?2 ORDER BY start_ts",
        )?;
        let rows = stmt.query_map([from.to_string(), to.to_string()], |r| {
            Ok(ExportVisit {
                day: day(r.get(0)?)?,
                start: r.get(1)?,
                end: r.get(2)?,
                browser: r.get(3)?,
                host: r.get(4)?,
                url: r.get(5)?,
                title: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Writes a consistent copy of the database to `path` (`VACUUM INTO`).
    pub fn backup_to(&self, path: &std::path::Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| crate::StorageError::Io(e.to_string()))?;
        }
        self.conn
            .execute("VACUUM INTO ?1", [path.to_string_lossy().as_ref()])?;
        Ok(())
    }

    /// Active time of apps whose category has `kind` on `day`.
    pub fn category_kind_ms(&self, day: LocalDay, kind: CategoryKind) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT COALESCE(SUM(t.ms), 0) FROM daily_app_totals t
             JOIN apps a ON a.id = t.app_id JOIN categories c ON c.id = a.category_id
             WHERE t.local_day = ?1 AND c.kind = ?2",
            [day.to_string(), kind.as_str().to_string()],
            |r| r.get(0),
        )?)
    }
}
