use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use tokio::sync::{Semaphore, mpsc, watch};

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

const DEFAULT_MAX_CONCURRENT_DOWNLOADS: usize = 2;

#[derive(Clone)]
pub struct DownloadCoordinator {
    inner: Arc<DownloadCoordinatorInner>,
}

struct DownloadCoordinatorInner {
    engine: Arc<dyn DownloadEngine>,
    active: Mutex<HashMap<String, ActiveDownload>>,
    next_generation: AtomicU64,
    download_slots: Arc<Semaphore>,
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
        Self::with_max_concurrent(engine, DEFAULT_MAX_CONCURRENT_DOWNLOADS)
    }

    fn with_max_concurrent(engine: impl DownloadEngine, max_concurrent: usize) -> Self {
        assert!(max_concurrent > 0, "download concurrency must be positive");
        Self {
            inner: Arc::new(DownloadCoordinatorInner {
                engine: Arc::new(engine),
                active: Mutex::new(HashMap::new()),
                next_generation: AtomicU64::new(1),
                download_slots: Arc::new(Semaphore::new(max_concurrent)),
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
        let DownloadRun {
            generation,
            mut cancellation,
        } = run;
        let mut sequence = 0;
        emit(event(&task.id, sequence, DownloadEventPayload::Queued));

        let permit = tokio::select! {
            biased;
            _ = cancellation.changed() => {
                sequence += 1;
                emit(event(&task.id, sequence, DownloadEventPayload::Cancelled));
                self.finish(&task.id, generation);
                return;
            }
            permit = Arc::clone(&self.inner.download_slots).acquire_owned() => {
                permit.expect("download semaphore should remain open")
            }
        };

        sequence += 1;
        emit(event(&task.id, sequence, DownloadEventPayload::Started));

        let (update_sender, mut update_receiver) = mpsc::unbounded_channel();
        let download = self
            .inner
            .engine
            .download(&task, cancellation, update_sender);
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
        drop(permit);
        self.finish(&task.id, generation);
    }

    fn finish(&self, task_id: &DownloadTaskId, generation: u64) {
        let mut active = self.active();
        if active
            .get(task_id.as_str())
            .is_some_and(|active| active.generation == generation)
        {
            active.remove(task_id.as_str());
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
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    use std::time::Duration;

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

    struct GatedEngine {
        starts: mpsc::UnboundedSender<String>,
        release: Arc<Semaphore>,
        active: Arc<AtomicUsize>,
        max_active: Arc<AtomicUsize>,
    }

    impl DownloadEngine for GatedEngine {
        fn download<'a>(
            &'a self,
            task: &'a DownloadTask,
            mut cancellation: watch::Receiver<bool>,
            _updates: mpsc::UnboundedSender<DownloadEngineUpdate>,
        ) -> DownloadFuture<'a> {
            Box::pin(async move {
                let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
                self.max_active.fetch_max(active, Ordering::SeqCst);
                let _ = self.starts.send(task.id.as_str().to_owned());

                let outcome = tokio::select! {
                    _ = cancellation.changed() => DownloadOutcome::Cancelled,
                    permit = self.release.acquire() => {
                        permit.expect("test release semaphore should remain open").forget();
                        DownloadOutcome::Completed
                    }
                };
                self.active.fetch_sub(1, Ordering::SeqCst);
                Ok(outcome)
            })
        }
    }

    fn task_with_id(id: &str) -> DownloadTask {
        DownloadTask::new(
            DownloadTaskId::new(id).expect("task ID should be valid"),
            "https://video.example/watch",
            "Title",
            "format-1",
            std::env::temp_dir()
                .join(format!("{id}.mp4"))
                .to_string_lossy(),
            "mp4",
            DownloadStreams::VideoOnly,
        )
        .expect("task should be valid")
    }

    fn task() -> DownloadTask {
        task_with_id("task-1")
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

    #[tokio::test]
    async fn limits_downloads_and_starts_the_next_task_in_order() {
        let (starts_sender, mut starts_receiver) = mpsc::unbounded_channel();
        let release = Arc::new(Semaphore::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));
        let coordinator = DownloadCoordinator::with_max_concurrent(
            GatedEngine {
                starts: starts_sender,
                release: Arc::clone(&release),
                active: Arc::new(AtomicUsize::new(0)),
                max_active: Arc::clone(&max_active),
            },
            2,
        );

        let mut handles = Vec::new();
        for id in ["task-1", "task-2", "task-3"] {
            let task = task_with_id(id);
            let run = coordinator.begin(&task.id).expect("task should begin");
            let coordinator = coordinator.clone();
            handles.push(tokio::spawn(async move {
                coordinator.run(task, run, |_| {}).await;
            }));
            tokio::task::yield_now().await;
        }

        let first = tokio::time::timeout(Duration::from_secs(1), starts_receiver.recv())
            .await
            .expect("first download should start")
            .expect("starts channel should remain open");
        let second = tokio::time::timeout(Duration::from_secs(1), starts_receiver.recv())
            .await
            .expect("second download should start")
            .expect("starts channel should remain open");
        assert_eq!([first.as_str(), second.as_str()], ["task-1", "task-2"]);
        assert!(starts_receiver.try_recv().is_err());

        release.add_permits(1);
        let third = tokio::time::timeout(Duration::from_secs(1), starts_receiver.recv())
            .await
            .expect("queued download should start")
            .expect("starts channel should remain open");
        assert_eq!(third, "task-3");

        release.add_permits(2);
        for handle in handles {
            handle.await.expect("download task should finish");
        }
        assert_eq!(max_active.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn cancels_a_download_while_it_is_waiting_for_a_slot() {
        let (starts_sender, mut starts_receiver) = mpsc::unbounded_channel();
        let release = Arc::new(Semaphore::new(0));
        let coordinator = DownloadCoordinator::with_max_concurrent(
            GatedEngine {
                starts: starts_sender,
                release: Arc::clone(&release),
                active: Arc::new(AtomicUsize::new(0)),
                max_active: Arc::new(AtomicUsize::new(0)),
            },
            1,
        );

        let first_task = task_with_id("task-1");
        let first_run = coordinator
            .begin(&first_task.id)
            .expect("first task should begin");
        let first_coordinator = coordinator.clone();
        let first = tokio::spawn(async move {
            first_coordinator.run(first_task, first_run, |_| {}).await;
        });
        assert_eq!(starts_receiver.recv().await.as_deref(), Some("task-1"));

        let queued_task = task_with_id("task-2");
        let queued_run = coordinator
            .begin(&queued_task.id)
            .expect("queued task should begin");
        let events = Arc::new(Mutex::new(Vec::new()));
        let emitted = Arc::clone(&events);
        let queued_coordinator = coordinator.clone();
        let queued = tokio::spawn(async move {
            queued_coordinator
                .run(queued_task, queued_run, move |event| {
                    emitted.lock().expect("events lock").push(event.payload);
                })
                .await;
        });

        tokio::task::yield_now().await;
        assert!(coordinator.cancel("task-2"));
        queued
            .await
            .expect("queued task should finish after cancellation");
        assert!(starts_receiver.try_recv().is_err());
        assert_eq!(
            *events.lock().expect("events lock"),
            vec![
                DownloadEventPayload::Queued,
                DownloadEventPayload::Cancelled
            ]
        );

        release.add_permits(1);
        first.await.expect("first task should finish");
    }
}
