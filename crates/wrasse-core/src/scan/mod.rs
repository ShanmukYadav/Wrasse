//! Whole-volume scanning (PLAN.md §5): a parallel read-only walk into a compact arena tree,
//! then a breakdown into the categories the UI shows.

pub mod breakdown;
pub mod cache;
pub mod hf;
pub mod tree;
pub mod walk;

pub use tree::{ScanStats, Tree};
pub use walk::{scan, Progress, ScanControl};
