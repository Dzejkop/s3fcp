use std::sync::Arc;

use crate::progress::ProgressTracker;

pub struct QuietProgressTracker;

impl QuietProgressTracker {
    pub fn dyn_new() -> Arc<dyn ProgressTracker> {
        Arc::new(QuietProgressTracker)
    }
}

impl ProgressTracker for QuietProgressTracker {
    fn reset(&self, _total_bytes: u64) {}
    fn increment(&self, _bytes: u64) {}
    fn finish(&self) {}
}
