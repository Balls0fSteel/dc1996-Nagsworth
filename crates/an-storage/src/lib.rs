//! SQLite storage. One actor thread owns the connection; everyone else talks
//! to it through a cloneable [`StorageHandle`].

mod db;
pub mod stats;

use std::path::Path;

use tokio::sync::{mpsc, oneshot};

pub use db::{AppRow, Category, Db, NagLogEntry};

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("migration: {0}")]
    Migration(#[from] rusqlite_migration::Error),
    #[error("io: {0}")]
    Io(String),
    #[error("{0} not found")]
    NotFound(&'static str),
    #[error("storage actor stopped")]
    Closed,
}

type Job = Box<dyn FnOnce(&mut Db) + Send>;

/// Cloneable handle to the storage actor.
#[derive(Clone, Debug)]
pub struct StorageHandle {
    tx: mpsc::UnboundedSender<Job>,
}

impl StorageHandle {
    /// Opens (or creates) the database at `path` and starts the actor thread.
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        Ok(Self::spawn(Db::open(path)?))
    }

    pub fn open_in_memory() -> Result<Self, StorageError> {
        Ok(Self::spawn(Db::open_in_memory()?))
    }

    fn spawn(mut db: Db) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel::<Job>();
        std::thread::Builder::new()
            .name("an-storage".into())
            .spawn(move || {
                while let Some(job) = rx.blocking_recv() {
                    job(&mut db);
                }
                tracing::debug!("storage actor stopped");
            })
            .expect("spawn storage thread");
        Self { tx }
    }

    /// Runs `f` on the storage thread and returns its result.
    pub async fn call<R, F>(&self, f: F) -> Result<R, StorageError>
    where
        F: FnOnce(&mut Db) -> Result<R, StorageError> + Send + 'static,
        R: Send + 'static,
    {
        let (done, result) = oneshot::channel();
        self.tx
            .send(Box::new(move |db| {
                let _ = done.send(f(db));
            }))
            .map_err(|_| StorageError::Closed)?;
        result.await.map_err(|_| StorageError::Closed)?
    }

    /// Like [`StorageHandle::call`] for code outside the async runtime.
    pub fn call_blocking<R, F>(&self, f: F) -> Result<R, StorageError>
    where
        F: FnOnce(&mut Db) -> Result<R, StorageError> + Send + 'static,
        R: Send + 'static,
    {
        let (done, result) = std::sync::mpsc::channel();
        self.tx
            .send(Box::new(move |db| {
                let _ = done.send(f(db));
            }))
            .map_err(|_| StorageError::Closed)?;
        result.recv().map_err(|_| StorageError::Closed)?
    }

    /// Fire-and-forget write; errors are logged.
    pub fn send<F>(&self, f: F)
    where
        F: FnOnce(&mut Db) -> Result<(), StorageError> + Send + 'static,
    {
        let _ = self.tx.send(Box::new(move |db| {
            if let Err(e) = f(db) {
                tracing::warn!("storage write failed: {e}");
            }
        }));
    }
}

#[cfg(test)]
mod tests {
    use an_core::{CategoryKind, LocalDay, ProcessInfo, SegmentKind};

    use super::*;

    fn process(exe: &str) -> ProcessInfo {
        ProcessInfo {
            pid: 1,
            exe_name: exe.into(),
            exe_path: Some(format!("C:\\{exe}")),
            is_elevated: false,
            package_id: None,
        }
    }

    fn day(s: &str) -> LocalDay {
        s.parse().unwrap()
    }

    #[test]
    fn apps_keep_their_category() {
        let mut db = Db::open_in_memory().unwrap();
        let a = db
            .get_or_create_app(&process("game.exe"), CategoryKind::Game, true, 10)
            .unwrap();
        assert_eq!(a.category_kind, CategoryKind::Game);
        assert!(a.is_game);
        // A second sighting with a different default does not recategorize.
        let b = db
            .get_or_create_app(&process("GAME.EXE"), CategoryKind::Other, false, 20)
            .unwrap();
        assert_eq!(a.id, b.id);
        assert_eq!(b.category_kind, CategoryKind::Game);
        assert_eq!(b.last_seen, 20);

        let work = db
            .categories()
            .unwrap()
            .into_iter()
            .find(|c| c.kind == CategoryKind::Work)
            .unwrap();
        let c = db
            .update_app(
                a.id,
                Some(work.id),
                Some(false),
                Some(Some("My game".into())),
            )
            .unwrap();
        assert_eq!(c.category_source, "user");
        assert_eq!(c.display_name.as_deref(), Some("My game"));
        db.set_display_name_if_empty(a.id, "Other").unwrap();
        assert_eq!(
            db.app(a.id).unwrap().unwrap().display_name.as_deref(),
            Some("My game")
        );
        assert_eq!(db.apps().unwrap().len(), 1);
    }

    #[test]
    fn segments_feed_daily_totals() {
        let mut db = Db::open_in_memory().unwrap();
        let d = day("2026-11-02");
        let app = db
            .get_or_create_app(&process("code.exe"), CategoryKind::Work, false, 0)
            .unwrap();
        let t = db.title_id("main.rs").unwrap();
        assert_eq!(db.title_id("main.rs").unwrap(), t);
        let s = db
            .open_segment(Some(app.id), Some(t), SegmentKind::Active, 1_000, d)
            .unwrap();
        db.extend_segment(s, 11_000).unwrap();
        db.extend_segment(s, 21_000).unwrap();
        // Going backwards is ignored.
        db.extend_segment(s, 5_000).unwrap();
        let idle = db
            .open_segment(None, None, SegmentKind::Idle, 21_000, d)
            .unwrap();
        db.extend_segment(idle, 81_000).unwrap();

        let apps = db.usage_by_app(d, d).unwrap();
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].ms, 20_000);
        let cats = db.usage_by_category(d, d).unwrap();
        assert_eq!(cats[0].kind, CategoryKind::Work);
        assert_eq!(db.category_kind_ms(d, CategoryKind::Work).unwrap(), 20_000);
        let kinds = db.kind_totals(d, d).unwrap();
        assert_eq!(kinds.len(), 2);
        let tl = db.timeline(d).unwrap();
        assert_eq!(tl.len(), 2);
        assert_eq!(tl[0].title.as_deref(), Some("main.rs"));
        assert_eq!(tl[1].kind, SegmentKind::Idle);
        assert_eq!(db.daily_usage(d, d).unwrap()[0].ms, 20_000);

        let exported = db.export_segments(d, d).unwrap();
        assert_eq!(exported.len(), 2);
        assert_eq!(exported[0].category.as_deref(), Some("Work"));
        assert_eq!(exported[0].title.as_deref(), Some("main.rs"));
        let backup = std::env::temp_dir().join(format!("an-backup-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&backup);
        db.backup_to(&backup).unwrap();
        let copy = Db::open(&backup).unwrap();
        assert_eq!(copy.usage_by_app(d, d).unwrap()[0].ms, 20_000);
        drop(copy);
        let _ = std::fs::remove_file(&backup);

        // Pruning drops segments but keeps totals.
        assert_eq!(db.prune(day("2026-11-03"), 0).unwrap(), 2);
        assert!(db.timeline(d).unwrap().is_empty());
        assert_eq!(db.usage_by_app(d, d).unwrap()[0].ms, 20_000);
    }

    #[test]
    fn web_visits_and_dedup() {
        let mut db = Db::open_in_memory().unwrap();
        let d = day("2026-11-02");
        let v = db
            .open_visit("chrome", "example.com", None, Some("Ex"), Some(3), 0, d)
            .unwrap();
        db.extend_visit(v, 30_000).unwrap();
        assert_eq!(db.usage_by_host(d, d).unwrap()[0].ms, 30_000);
        assert_eq!(db.export_visits(d, d).unwrap()[0].host, "example.com");
        assert!(db.mark_ext_event("c1", 5, 3).unwrap());
        assert!(!db.mark_ext_event("c1", 5, 3).unwrap());
    }

    #[test]
    fn kv_nags_and_rule_state() {
        let mut db = Db::open_in_memory().unwrap();
        assert_eq!(db.kv_get("token").unwrap(), None);
        db.kv_set("token", "a").unwrap();
        db.kv_set("token", "b").unwrap();
        assert_eq!(db.kv_get("token").unwrap().as_deref(), Some("b"));
        db.log_nag("r", "raise", Some("toast"), None, 100).unwrap();
        db.log_nag("r", "dismiss", None, Some("skipped"), 200)
            .unwrap();
        let nags = db.nags(0, 10).unwrap();
        assert_eq!(nags.len(), 2);
        assert_eq!(nags[0].kind, "dismiss");
        db.save_rule_states(&[("r".into(), "{}".into())], 1)
            .unwrap();
        db.save_rule_states(&[("r".into(), "{\"a\":1}".into())], 2)
            .unwrap();
        assert_eq!(db.rule_states().unwrap()[0].1, "{\"a\":1}");
        let c = db
            .create_category("Study", CategoryKind::Work, None)
            .unwrap();
        assert_eq!(db.categories().unwrap().last().unwrap(), &c);
    }

    #[tokio::test]
    async fn actor_round_trip() {
        let h = StorageHandle::open_in_memory().unwrap();
        h.call(|db| db.kv_set("k", "v")).await.unwrap();
        let v = h.call(|db| db.kv_get("k")).await.unwrap();
        assert_eq!(v.as_deref(), Some("v"));
        let h2 = h.clone();
        let v = tokio::task::spawn_blocking(move || h2.call_blocking(|db| db.kv_get("k")))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(v.as_deref(), Some("v"));
    }

    #[test]
    fn opens_file_db_twice() {
        let dir = std::env::temp_dir().join(format!("an-storage-test-{}", std::process::id()));
        let path = dir.join("test.db");
        {
            let mut db = Db::open(&path).unwrap();
            db.kv_set("x", "1").unwrap();
        }
        let db = Db::open(&path).unwrap();
        assert_eq!(db.kv_get("x").unwrap().as_deref(), Some("1"));
        drop(db);
        let _ = std::fs::remove_dir_all(dir);
    }
}
