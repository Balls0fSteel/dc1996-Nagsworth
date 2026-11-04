//! Web time from browser-extension tab events.
//!
//! The extension reports which tab is active in each browser. Web time only
//! accrues while that browser is the foreground app and the user is not idle,
//! so a video left open behind another window does not count.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use ns_core::config::StoreUrl;
use ns_core::{
    Activity, Clock, Event, EventBus, LocalDay, SegmentKind, TimeOfDay, Timestamp, WebEvent,
};
use ns_storage::StorageHandle;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, watch};

use crate::categorize::{browser_family, is_browser};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtEventKind {
    TabActivated,
    TabUpdated,
    TabRemoved,
    /// No window of this browser has focus any more.
    FocusLost,
    Heartbeat,
}

/// One tab event as the extension sends it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtEvent {
    pub browser: String,
    pub ts: Timestamp,
    pub kind: ExtEventKind,
    #[serde(default)]
    pub tab_id: i64,
    #[serde(default)]
    pub window_id: Option<i64>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub incognito: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ActiveTab {
    tab_id: i64,
    url: String,
    host: String,
    title: Option<String>,
}

/// The host of an http(s) URL, without port or credentials.
pub fn host_of(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host_port = authority.rsplit('@').next()?;
    let host = if host_port.starts_with('[') {
        host_port.split(']').next().map(|h| format!("{h}]"))?
    } else {
        host_port.split(':').next()?.to_string()
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    (!host.is_empty()).then_some(host)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebAction {
    Open {
        browser: String,
        host: String,
        url: Option<String>,
        title: Option<String>,
        tab_id: i64,
        start: Timestamp,
        day: LocalDay,
    },
    Extend {
        end: Timestamp,
    },
    Close {
        end: Timestamp,
    },
}

#[derive(Clone, Debug)]
struct Visit {
    browser: String,
    host: String,
    url: Option<String>,
    end: Timestamp,
    last_flush: Timestamp,
}

#[derive(Debug)]
pub struct WebSegmenter {
    store_url: StoreUrl,
    day_start: TimeOfDay,
    flush_ms: i64,
    tabs: HashMap<String, Option<ActiveTab>>,
    last_browser: Option<String>,
    current: Option<Visit>,
}

impl WebSegmenter {
    pub fn new(store_url: StoreUrl, day_start: TimeOfDay) -> Self {
        Self {
            store_url,
            day_start,
            flush_ms: 10_000,
            tabs: HashMap::new(),
            last_browser: None,
            current: None,
        }
    }

    pub fn on_ext(&mut self, e: &ExtEvent) {
        let browser = e.browser.to_ascii_lowercase();
        match e.kind {
            ExtEventKind::TabActivated | ExtEventKind::TabUpdated | ExtEventKind::Heartbeat => {
                let tab = e.url.as_deref().and_then(|url| {
                    host_of(url).map(|host| ActiveTab {
                        tab_id: e.tab_id,
                        url: url.to_string(),
                        host,
                        title: e.title.clone(),
                    })
                });
                // An update of a background tab does not change the active one.
                let is_active_update = e.kind != ExtEventKind::TabUpdated
                    || self
                        .tabs
                        .get(&browser)
                        .and_then(|t| t.as_ref())
                        .is_none_or(|t| t.tab_id == e.tab_id);
                if is_active_update {
                    self.tabs.insert(browser.clone(), tab);
                }
            }
            ExtEventKind::TabRemoved => {
                if let Some(slot) = self.tabs.get_mut(&browser)
                    && slot.as_ref().is_some_and(|t| t.tab_id == e.tab_id)
                {
                    *slot = None;
                }
            }
            ExtEventKind::FocusLost => {
                self.tabs.insert(browser.clone(), None);
            }
        }
        self.last_browser = Some(browser);
    }

    /// The tab that is being looked at right now, if any.
    fn target(&self, activity: &Activity) -> Option<(String, ActiveTab)> {
        if activity.kind != Some(SegmentKind::Active) {
            return None;
        }
        let app = activity.app.as_ref()?;
        if !is_browser(&app.exe_name) {
            return None;
        }
        let family = browser_family(&app.exe_name);
        let browser = family
            .filter(|f| self.tabs.contains_key(*f))
            .map(str::to_string)
            .or_else(|| self.last_browser.clone())?;
        let tab = self.tabs.get(&browser)?.clone()?;
        Some((browser, tab))
    }

    pub fn on_tick(
        &mut self,
        activity: &Activity,
        now: Timestamp,
        offset_s: i32,
    ) -> Vec<WebAction> {
        let mut out = Vec::new();
        let target = self.target(activity);
        let target_url = |t: &ActiveTab| match self.store_url {
            StoreUrl::Full => Some(t.url.clone()),
            StoreUrl::Host => None,
        };
        let same = match (&self.current, &target) {
            (Some(v), Some((b, t))) => {
                v.browser == *b && v.host == t.host && v.url == target_url(t)
            }
            _ => false,
        };
        if same {
            let v = self.current.as_mut().expect("checked above");
            v.end = now;
            if now - v.last_flush >= self.flush_ms {
                v.last_flush = now;
                out.push(WebAction::Extend { end: now });
            }
            return out;
        }
        if let Some(v) = self.current.take() {
            out.push(WebAction::Close {
                end: v.end.max(now),
            });
        }
        if let Some((browser, tab)) = target {
            let url = target_url(&tab);
            out.push(WebAction::Open {
                browser: browser.clone(),
                host: tab.host.clone(),
                url: url.clone(),
                title: tab.title.clone(),
                tab_id: tab.tab_id,
                start: now,
                day: LocalDay::from_ts(now, offset_s, self.day_start),
            });
            self.current = Some(Visit {
                browser,
                host: tab.host,
                url,
                end: now,
                last_flush: now,
            });
        }
        out
    }
}

/// Runs a [`WebSegmenter`] once per second against storage and the bus.
pub struct WebTask {
    pub segmenter: WebSegmenter,
    pub storage: StorageHandle,
    pub bus: EventBus,
    pub clock: Arc<dyn Clock>,
    pub activity: watch::Receiver<Activity>,
}

impl WebTask {
    pub async fn run(mut self, mut events: mpsc::Receiver<ExtEvent>) {
        let mut ticker = tokio::time::interval(Duration::from_secs(1));
        let mut current: Option<i64> = None;
        loop {
            tokio::select! {
                e = events.recv() => match e {
                    Some(e) => self.segmenter.on_ext(&e),
                    None => break,
                },
                _ = ticker.tick() => {
                    let now = self.clock.now();
                    let activity = self.activity.borrow().clone();
                    let actions = self.segmenter.on_tick(&activity, now, self.clock.offset_s(now));
                    for a in actions {
                        self.apply(a, &mut current).await;
                    }
                }
            }
        }
    }

    async fn apply(&self, action: WebAction, current: &mut Option<i64>) {
        match action {
            WebAction::Open {
                browser,
                host,
                url,
                title,
                tab_id,
                start,
                day,
            } => {
                let (b, h, u, t) = (browser.clone(), host.clone(), url.clone(), title.clone());
                match self
                    .storage
                    .call(move |db| {
                        db.open_visit(&b, &h, u.as_deref(), t.as_deref(), Some(tab_id), start, day)
                    })
                    .await
                {
                    Ok(id) => {
                        *current = Some(id);
                        self.bus.publish(Event::Web(WebEvent::VisitStarted {
                            id,
                            browser,
                            host,
                            url,
                            title,
                            start,
                        }));
                    }
                    Err(e) => tracing::warn!("cannot open web visit: {e}"),
                }
            }
            WebAction::Extend { end } => {
                if let Some(id) = *current {
                    self.storage.send(move |db| db.extend_visit(id, end));
                }
            }
            WebAction::Close { end } => {
                if let Some(id) = current.take() {
                    self.storage.send(move |db| db.extend_visit(id, end));
                    self.bus
                        .publish(Event::Web(WebEvent::VisitEnded { id, end }));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ns_core::{AppRef, CategoryKind};

    use super::*;

    #[test]
    fn extracts_hosts() {
        assert_eq!(
            host_of("https://Example.com/a?b").as_deref(),
            Some("example.com")
        );
        assert_eq!(
            host_of("http://u:p@host.io:8080/x").as_deref(),
            Some("host.io")
        );
        assert_eq!(host_of("https://[::1]:3000/").as_deref(), Some("[::1]"));
        assert_eq!(host_of("chrome://newtab"), None);
        assert_eq!(host_of("about:blank"), None);
    }

    fn activity(exe: &str, kind: SegmentKind) -> Activity {
        Activity {
            kind: Some(kind),
            app: Some(AppRef {
                id: 1,
                exe_name: exe.into(),
                category: CategoryKind::Web,
                is_game: false,
            }),
            ..Activity::default()
        }
    }

    fn tab(kind: ExtEventKind, tab_id: i64, url: &str) -> ExtEvent {
        ExtEvent {
            browser: "chrome".into(),
            ts: 0,
            kind,
            tab_id,
            window_id: Some(1),
            url: Some(url.into()),
            title: None,
            incognito: false,
        }
    }

    #[test]
    fn accrues_only_while_browser_in_front() {
        let mut w = WebSegmenter::new(StoreUrl::Host, TimeOfDay::MIDNIGHT);
        w.on_ext(&tab(ExtEventKind::TabActivated, 1, "https://a.com/x"));
        let chrome = activity("chrome.exe", SegmentKind::Active);
        let a = w.on_tick(&chrome, 0, 0);
        assert!(matches!(&a[0], WebAction::Open { host, url: None, .. } if host == "a.com"));
        // Same host, other path: same visit.
        w.on_ext(&tab(ExtEventKind::TabUpdated, 1, "https://a.com/y"));
        assert!(w.on_tick(&chrome, 1000, 0).is_empty());
        // A background tab update changes nothing.
        w.on_ext(&tab(ExtEventKind::TabUpdated, 2, "https://b.com/"));
        assert!(w.on_tick(&chrome, 2000, 0).is_empty());
        // Another app in front closes the visit.
        let a = w.on_tick(&activity("code.exe", SegmentKind::Active), 3000, 0);
        assert_eq!(a, vec![WebAction::Close { end: 3000 }]);
        // Back to the browser, then idle.
        w.on_ext(&tab(ExtEventKind::TabActivated, 2, "https://b.com/"));
        let a = w.on_tick(&chrome, 4000, 0);
        assert!(matches!(&a[0], WebAction::Open { host, .. } if host == "b.com"));
        let a = w.on_tick(&activity("chrome.exe", SegmentKind::Idle), 5000, 0);
        assert_eq!(a, vec![WebAction::Close { end: 5000 }]);
        // Closing the active tab leaves nothing to track.
        w.on_ext(&tab(ExtEventKind::TabRemoved, 2, ""));
        assert!(w.on_tick(&chrome, 6000, 0).is_empty());
    }
}
