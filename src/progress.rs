pub mod bar;
pub mod logged;
pub mod quiet;

pub trait ProgressTracker: Send + Sync {
    fn reset(&self, total_bytes: u64);
    fn increment(&self, bytes: u64);
    fn finish(&self);
}
