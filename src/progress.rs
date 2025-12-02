use indicatif::{ProgressBar, ProgressStyle};
use std::sync::Arc;

pub struct ProgressTracker {
    bar: Option<ProgressBar>,
}

impl ProgressTracker {
    pub fn new(total_bytes: u64, quiet: bool) -> Arc<Self> {
        let bar = if quiet {
            None
        } else {
            let pb = ProgressBar::new(total_bytes);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
                    .expect("Invalid progress bar template")
                    .progress_chars("#>-"),
            );
            Some(pb)
        };

        Arc::new(Self { bar })
    }

    pub fn increment(&self, bytes: u64) {
        if let Some(ref bar) = self.bar {
            bar.inc(bytes);
        }
    }

    pub fn finish(&self) {
        if let Some(ref bar) = self.bar {
            bar.finish_with_message("Download complete");
        }
    }
}
