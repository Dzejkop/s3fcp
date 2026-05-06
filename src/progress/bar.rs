use indicatif::{ProgressBar, ProgressStyle};
use std::sync::Arc;

use crate::progress::ProgressTracker;

#[derive(Clone)]
pub struct ProgressBarTracker {
    bar: ProgressBar,
}

impl ProgressBarTracker {
    #[must_use]
    pub fn dyn_new() -> Arc<dyn ProgressTracker> {
        let bar = ProgressBar::new(0);
        if let Ok(style) = ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        {
            bar.set_style(style.progress_chars("#>-"));
        }

        Arc::new(Self { bar })
    }
}

impl ProgressTracker for ProgressBarTracker {
    fn reset(&self, total_bytes: u64) {
        self.bar.set_length(total_bytes);
    }

    fn increment(&self, bytes: u64) {
        self.bar.inc(bytes);
    }

    fn finish(&self) {
        self.bar.finish_with_message("Download complete");
    }
}
