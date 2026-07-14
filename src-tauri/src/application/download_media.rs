use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use tokio::sync::{mpsc, watch};

use crate::domain::{
    DownloadEvent, DownloadEventPayload, DownloadFailure, DownloadProgress, DownloadTask,
    DownloadTaskId,
};

pub type DownloadFuture<'a> =
    Pin<Box<dyn Future<Output = Result<DownloadOutcome, DownloadFailure>> + Send + 'a>>;

pub trait DownloadEngine: Send + Sync + 'static {
    fn download<'a>(
        &'a self,
        task: &'a DownloadTask,
        cancellation: watch::Receiver<bool>,
        updates: mpsc::UnboundedSender<DownloadEngineUpdate>,
    ) -> DownloadFuture<'a>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DownloadEngineUpdate {
    Progress(DownloadProgress),
    Processing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadOutcome {
    Completed,
    Cancelled,
}

#[derive(Clone)]
pub struct DownloadCoordinator {
    inner: Arc<DownloadCoordinatorInner>,
}

struct DownloadCoordinatorInner {
    engine: Arc<dyn DownloadEngine>,
    active: Mutex<HashMap<String, ActiveDownload>>,
    next_generation: AtomicU64,
}

struct ActiveDownload {
    generation: u64,
    cancellation: watch::Sender<bool>,
}

pub struct DownloadRun {
    generation: u64,
    cancellation: watch::Receiver<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartDownloadError {
    AlreadyRunning,
}

impl DownloadCoordinator {
    pub fn new(engine: impl DownloadEngine) -> Self {
        Self {
            inner: Arc::new(DownloadCoordinatorInner {
                engine: Arc::new(engine),
                active: Mutex::new(HashMap::new()),
                next_generation: AtomicU64::new(1),
            }),
        }
    }

    pub fn begin(&self, task_id: &DownloadTaskId) -> Result<DownloadRun, StartDownloadError> {
        let mut active = self.active();
        if active.contains_key(task_id.as_str()) {
            return Err(StartDownloadError::AlreadyRunning);
        }

        let generation = self.inner.next_generation.fetch_add(1, Ordering::Relaxed);
        let (cancellation, cancellation_receiver) = watch::channel(false);
        active.insert(
            task_id.as_str().to_owned(),
            ActiveDownload {
                generation,
                cancellation,
            },
        );
        Ok(DownloadRun {
            generation,
            cancellation: cancellation_receiver,
        })
    }

    pub fn cancel(&self, task_id: &str) -> bool {
        self.active()
            .get(task_id)
            .is_some_and(|download| download.cancellation.send(true).is_ok())
    }

    pub async fn run(
        &self,
        task: DownloadTask,
        run: DownloadRun,
        mut emit: impl FnMut(DownloadEvent),
    ) {
        let mut sequence = 0;
        emit(event(&task.id, sequence, DownloadEventPayload::Queued));
        sequence += 1;
        emit(event(&task.id, sequence, DownloadEventPayload::Started));

        let (update_sender, mut update_receiver) = mpsc::unbounded_channel();
        let download = self
            .inner
            .engine
            .download(&task, run.cancellation, update_sender);
        tokio::pin!(download);
        let mut updates_open = true;

        let payload = loop {
            tokio::select! {
                update = update_receiver.recv(), if updates_open => {
                    if let Some(update) = update {
                        sequence += 1;
                        let payload = match update {
                            DownloadEngineUpdate::Progress(progress) => {
                                DownloadEventPayload::Progress { progress }
                            }
                            DownloadEngineUpdate::Processing => DownloadEventPayload::Processing,
                        };
                        emit(event(&task.id, sequence, payload));
                    } else {
                        updates_open = false;
                    }
                }
                result = &mut download => {
                    break match result {
                        Ok(DownloadOutcome::Completed) => DownloadEventPayload::Completed,
                        Ok(DownloadOutcome::Cancelled) => DownloadEventPayload::Cancelled,
                        Err(error) => DownloadEventPayload::Failed { error },
                    };
                }
            }
        };

        sequence += 1;
        emit(event(&task.id, sequence, payload));
        let mut active = self.active();
        if active
            .get(task.id.as_str())
            .is_some_and(|active| active.generation == run.generation)
        {
            active.remove(task.id.as_str());
        }
    }

    fn active(&self) -> std::sync::MutexGuard<'_, HashMap<String, ActiveDownload>> {
        self.inner
            .active
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

fn event(task_id: &DownloadTaskId, sequence: u64, payload: DownloadEventPayload) -> DownloadEvent {
    DownloadEvent {
        task_id: task_id.clone(),
        sequence,
        payload,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::domain::DownloadStreams;

    struct StubEngine {
        outcome: DownloadOutcome,
    }

    impl DownloadEngine for StubEngine {
        fn download<'a>(
            &'a self,
            _task: &'a DownloadTask,
            _cancellation: watch::Receiver<bool>,
            updates: mpsc::UnboundedSender<DownloadEngineUpdate>,
        ) -> DownloadFuture<'a> {
            Box::pin(async move {
                let _ = updates.send(DownloadEngineUpdate::Progress(DownloadProgress {
                    downloaded_bytes: 50,
                    total_bytes: Some(100),
                    speed_bytes_per_second: Some(25),
                    eta_seconds: Some(2),
                }));
                Ok(self.outcome)
            })
        }
    }

    fn task() -> DownloadTask {
        DownloadTask::new(
            DownloadTaskId::new("task-1").expect("task ID should be valid"),
            "https://video.example/watch",
            "Title",
            "format-1",
            std::env::temp_dir().join("video.mp4").to_string_lossy(),
            "mp4",
            DownloadStreams::VideoOnly,
        )
        .expect("task should be valid")
    }

    #[tokio::test]
    async fn emits_a_sequenced_download_lifecycle() {
        let coordinator = DownloadCoordinator::new(StubEngine {
            outcome: DownloadOutcome::Completed,
        });
        let task = task();
        let run = coordinator.begin(&task.id).expect("task should begin");
        let events = Arc::new(Mutex::new(Vec::new()));
        let emitted = Arc::clone(&events);

        coordinator
            .run(task, run, move |event| {
                emitted.lock().expect("events lock").push(event);
            })
            .await;

        let events = events.lock().expect("events lock");
        assert_eq!(events.first().expect("queued event").sequence, 0);
        assert!(matches!(
            events.last().expect("completed event").payload,
            DownloadEventPayload::Completed
        ));
        assert!(
            events
                .windows(2)
                .all(|pair| pair[0].sequence < pair[1].sequence)
        );
    }

    #[test]
    fn prevents_duplicate_active_task_ids() {
        let coordinator = DownloadCoordinator::new(StubEngine {
            outcome: DownloadOutcome::Completed,
        });
        let task = task();
        let _run = coordinator.begin(&task.id).expect("task should begin");

        assert!(matches!(
            coordinator.begin(&task.id),
            Err(StartDownloadError::AlreadyRunning)
        ));
        assert!(coordinator.cancel(task.id.as_str()));
    }
}
