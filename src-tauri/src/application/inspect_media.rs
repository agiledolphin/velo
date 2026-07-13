use std::sync::Arc;

use crate::domain::{InspectError, MediaInfo};

pub trait MediaEngine: Send + Sync + 'static {
    fn inspect(&self, url: &str) -> Result<MediaInfo, InspectError>;
}

pub struct AppState {
    engine: Arc<dyn MediaEngine>,
}

impl AppState {
    pub fn new(engine: impl MediaEngine) -> Self {
        Self {
            engine: Arc::new(engine),
        }
    }

    pub fn inspect(&self, url: &str) -> Result<MediaInfo, InspectError> {
        self.engine.inspect(url)
    }
}
