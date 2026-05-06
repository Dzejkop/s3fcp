use std::sync::{Arc, Mutex};

use crate::progress::ProgressTracker;
use human_bytes::human_bytes;

// Default is 1 GiB
const DEFAULT_PROGRESS_THRESHOLD: u64 = 1024 * 1024 * 1024;

pub struct LoggedProgressTracker {
    // The threshold after which we log
    threshold: u64,
    inner: Mutex<Inner>,
}

#[derive(Default, Clone)]
struct Inner {
    total_bytes: u64,
    current_bytes: u64,
    // How many bytes have been downloaded under threshold
    current_bytes_part: u64,
}

impl LoggedProgressTracker {
    #[must_use]
    pub fn dyn_new() -> Arc<dyn ProgressTracker> {
        Arc::new(Self {
            threshold: DEFAULT_PROGRESS_THRESHOLD,
            inner: Mutex::new(Inner::default()),
        })
    }
}

impl ProgressTracker for LoggedProgressTracker {
    fn reset(&self, total_bytes: u64) {
        self.inner
            .lock()
            .expect("poisoned tracker lock")
            .total_bytes = total_bytes;

        eprintln!("Starting download of {}", human_bytes(total_bytes as f64));
    }

    fn increment(&self, bytes: u64) {
        let mut inner = self.inner.lock().expect("poisoned tracker lock");

        inner.current_bytes += bytes;
        inner.current_bytes_part += bytes;

        if inner.current_bytes_part > self.threshold {
            let percentage = (inner.current_bytes as f64 / inner.total_bytes as f64) * 100.0;
            eprintln!(
                "Downloaded {} / {} bytes ({:.2}%)",
                human_bytes(inner.current_bytes as f64),
                human_bytes(inner.total_bytes as f64),
                percentage
            );
            inner.current_bytes_part = 0;
        }
    }

    fn finish(&self) {
        // do nothing
    }
}
