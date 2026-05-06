use std::sync::Arc;

use crate::progress::ProgressTracker;

pub struct QuietProgressTracker;

impl QuietProgressTracker {
    #[must_use]
    pub fn dyn_new() -> Arc<dyn ProgressTracker> {
        Arc::new(Self)
    }
}

impl ProgressTracker for QuietProgressTracker {
    fn reset(&self, _total_bytes: u64) {}
    fn increment(&self, _bytes: u64) {}
    fn finish(&self) {}
}
