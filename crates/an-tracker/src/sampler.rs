use std::sync::Arc;
use std::time::Duration;

use an_core::{Clock, Sample};
use an_platform::Platform;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;

/// Polls the platform every `interval` and sends the samples on.
pub struct Sampler;

impl Sampler {
    /// Runs until the receiver is dropped.
    pub fn spawn(
        platform: Arc<dyn Platform>,
        clock: Arc<dyn Clock>,
        interval: Duration,
        tx: mpsc::Sender<Sample>,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                let platform = platform.clone();
                let ts = clock.now();
                // Win32 queries are fast but can stall on a hung window.
                let sample = match tokio::task::spawn_blocking(move || platform.sample(ts)).await {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!("sampler task failed: {e}");
                        continue;
                    }
                };
                if tx.send(sample).await.is_err() {
                    break;
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use an_core::MockClock;
    use an_platform::MockPlatform;
    use an_platform::mock::{sample, window};

    use super::*;

    #[tokio::test]
    async fn forwards_platform_samples() {
        let platform = Arc::new(MockPlatform::scripted(vec![
            sample(1, Some(window("a.exe", "A")), 0),
            sample(2, Some(window("b.exe", "B")), 0),
        ]));
        let (tx, mut rx) = mpsc::channel(4);
        let h = Sampler::spawn(
            platform,
            Arc::new(MockClock::new(0)),
            Duration::from_millis(5),
            tx,
        );
        assert_eq!(rx.recv().await.unwrap().ts, 1);
        assert_eq!(rx.recv().await.unwrap().ts, 2);
        drop(rx);
        h.await.unwrap();
    }
}
