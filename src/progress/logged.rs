use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

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
    started_at: Option<Instant>,
    last_logged_at: Option<Instant>,
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

#[allow(clippy::cast_precision_loss)]
const fn bytes_to_f64(bytes: u64) -> f64 {
    bytes as f64
}

fn average_speed(bytes: u64, elapsed: Duration) -> f64 {
    let seconds = elapsed.as_secs_f64();
    if seconds > 0.0 {
        bytes_to_f64(bytes) / seconds
    } else {
        0.0
    }
}

fn estimated_remaining(
    current_bytes: u64,
    total_bytes: u64,
    bytes_per_second: f64,
) -> Option<Duration> {
    if bytes_per_second <= 0.0 || current_bytes >= total_bytes {
        return None;
    }

    Some(Duration::from_secs_f64(
        bytes_to_f64(total_bytes - current_bytes) / bytes_per_second,
    ))
}

impl ProgressTracker for LoggedProgressTracker {
    fn reset(&self, total_bytes: u64) {
        let mut inner = self.inner.lock().expect("poisoned tracker lock");
        inner.total_bytes = total_bytes;
        inner.current_bytes = 0;
        let now = Instant::now();
        inner.current_bytes_part = 0;
        inner.started_at = Some(now);
        inner.last_logged_at = Some(now);
        drop(inner);

        eprintln!(
            "Starting download of {}",
            human_bytes(bytes_to_f64(total_bytes))
        );
    }

    fn increment(&self, bytes: u64) {
        let mut inner = self.inner.lock().expect("poisoned tracker lock");

        inner.current_bytes += bytes;
        inner.current_bytes_part += bytes;

        if inner.current_bytes_part > self.threshold {
            let now = Instant::now();
            let elapsed = inner
                .started_at
                .map_or(Duration::ZERO, |start| now.duration_since(start));
            let part_elapsed = inner
                .last_logged_at
                .map_or(elapsed, |last_log| now.duration_since(last_log));

            let average_bytes_per_second = average_speed(inner.current_bytes, elapsed);
            let part_bytes_per_second = average_speed(inner.current_bytes_part, part_elapsed);

            let remaining = estimated_remaining(
                inner.current_bytes,
                inner.total_bytes,
                average_bytes_per_second,
            )
            .map_or_else(
                || "unknown".to_string(),
                |duration| humantime::format_duration(duration).to_string(),
            );

            let percentage = if inner.total_bytes > 0 {
                (bytes_to_f64(inner.current_bytes) / bytes_to_f64(inner.total_bytes)) * 100.0
            } else {
                0.0
            };

            eprintln!(
                "Downloaded {} / {} ({:.2}%); avg {}/s, recent {}/s; elapsed {}; ETA {}",
                human_bytes(bytes_to_f64(inner.current_bytes)),
                human_bytes(bytes_to_f64(inner.total_bytes)),
                percentage,
                human_bytes(average_bytes_per_second),
                human_bytes(part_bytes_per_second),
                humantime::format_duration(elapsed),
                remaining
            );
            inner.current_bytes_part = 0;
            inner.last_logged_at = Some(now);
        }
    }

    fn finish(&self) {
        let inner = self.inner.lock().expect("poisoned tracker lock");
        let current_bytes = inner.current_bytes;
        let elapsed = inner
            .started_at
            .map_or(Duration::ZERO, |start| start.elapsed());
        drop(inner);

        let bytes_per_second = average_speed(current_bytes, elapsed);

        eprintln!(
            "Download complete: {} in {} (avg {}/s)",
            human_bytes(bytes_to_f64(current_bytes)),
            humantime::format_duration(elapsed),
            human_bytes(bytes_per_second)
        );
    }
}
