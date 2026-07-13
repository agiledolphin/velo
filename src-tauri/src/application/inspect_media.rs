use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use tokio::sync::watch;

use crate::domain::{InspectError, MediaInfo};

pub type InspectFuture<'a> =
    Pin<Box<dyn Future<Output = Result<MediaInfo, InspectError>> + Send + 'a>>;

pub trait MediaEngine: Send + Sync + 'static {
    fn inspect<'a>(&'a self, url: &'a str) -> InspectFuture<'a>;
}

pub struct AppState {
    engine: Arc<dyn MediaEngine>,
    active_inspections: Mutex<HashMap<String, ActiveInspection>>,
    next_generation: AtomicU64,
}

struct ActiveInspection {
    generation: u64,
    cancellation: watch::Sender<bool>,
}

impl AppState {
    pub fn new(engine: impl MediaEngine) -> Self {
        Self {
            engine: Arc::new(engine),
            active_inspections: Mutex::new(HashMap::new()),
            next_generation: AtomicU64::new(1),
        }
    }

    pub async fn inspect(&self, request_id: &str, url: &str) -> Result<MediaInfo, InspectError> {
        if !is_valid_request_id(request_id) {
            return Err(InspectError::invalid_request());
        }

        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
        let (cancellation, mut cancellation_receiver) = watch::channel(false);
        let previous = self.active_inspections().insert(
            request_id.to_owned(),
            ActiveInspection {
                generation,
                cancellation,
            },
        );
        if let Some(previous) = previous {
            let _ = previous.cancellation.send(true);
        }

        let result = tokio::select! {
            biased;
            _ = cancellation_receiver.changed() => Err(InspectError::cancelled()),
            result = self.engine.inspect(url) => result,
        };

        let mut active = self.active_inspections();
        if active
            .get(request_id)
            .is_some_and(|inspection| inspection.generation == generation)
        {
            active.remove(request_id);
        }
        result
    }

    pub fn cancel(&self, request_id: &str) -> bool {
        if !is_valid_request_id(request_id) {
            return false;
        }

        self.active_inspections()
            .get(request_id)
            .is_some_and(|inspection| inspection.cancellation.send(true).is_ok())
    }

    fn active_inspections(&self) -> std::sync::MutexGuard<'_, HashMap<String, ActiveInspection>> {
        self.active_inspections
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

fn is_valid_request_id(request_id: &str) -> bool {
    !request_id.is_empty()
        && request_id.len() <= 64
        && request_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

#[cfg(test)]
mod tests {
    use std::{
        future::pending,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    };

    use tokio::sync::Notify;

    use super::*;

    struct PendingEngine {
        started: Arc<Notify>,
        future_dropped: Arc<AtomicBool>,
    }

    struct DropMarker(Arc<AtomicBool>);

    impl Drop for DropMarker {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    impl MediaEngine for PendingEngine {
        fn inspect<'a>(&'a self, _url: &'a str) -> InspectFuture<'a> {
            Box::pin(async move {
                let _drop_marker = DropMarker(Arc::clone(&self.future_dropped));
                self.started.notify_one();
                pending().await
            })
        }
    }

    #[tokio::test]
    async fn cancels_and_drops_the_active_inspection() {
        let started = Arc::new(Notify::new());
        let future_dropped = Arc::new(AtomicBool::new(false));
        let state = Arc::new(AppState::new(PendingEngine {
            started: Arc::clone(&started),
            future_dropped: Arc::clone(&future_dropped),
        }));
        let inspection_state = Arc::clone(&state);
        let inspection = tokio::spawn(async move {
            inspection_state
                .inspect("request-1", "https://video.example/watch")
                .await
        });

        tokio::time::timeout(Duration::from_secs(1), started.notified())
            .await
            .expect("inspection should start");
        assert!(state.cancel("request-1"));

        let error = inspection
            .await
            .expect("inspection task should finish")
            .expect_err("cancelled inspection should return an error");
        assert_eq!(error.code, "inspect_cancelled");
        assert!(future_dropped.load(Ordering::SeqCst));
        assert!(!state.cancel("request-1"));
    }

    #[tokio::test]
    async fn rejects_invalid_request_ids_without_starting_the_engine() {
        let state = AppState::new(PendingEngine {
            started: Arc::new(Notify::new()),
            future_dropped: Arc::new(AtomicBool::new(false)),
        });

        let error = state
            .inspect("invalid request id", "https://video.example/watch")
            .await
            .expect_err("invalid request id should be rejected");

        assert_eq!(error.code, "invalid_request");
        assert!(!state.cancel("invalid request id"));
    }

    #[tokio::test]
    async fn a_reused_request_id_cancels_only_the_previous_generation() {
        let started = Arc::new(Notify::new());
        let state = Arc::new(AppState::new(PendingEngine {
            started: Arc::clone(&started),
            future_dropped: Arc::new(AtomicBool::new(false)),
        }));

        let first_state = Arc::clone(&state);
        let first = tokio::spawn(async move {
            first_state
                .inspect("shared-request", "https://video.example/first")
                .await
        });
        started.notified().await;

        let second_state = Arc::clone(&state);
        let second = tokio::spawn(async move {
            second_state
                .inspect("shared-request", "https://video.example/second")
                .await
        });
        started.notified().await;

        let first_error = first
            .await
            .expect("first task should finish")
            .expect_err("first task should be replaced");
        assert_eq!(first_error.code, "inspect_cancelled");
        assert!(state.cancel("shared-request"));

        let second_error = second
            .await
            .expect("second task should finish")
            .expect_err("second task should be cancelled explicitly");
        assert_eq!(second_error.code, "inspect_cancelled");
    }
}
