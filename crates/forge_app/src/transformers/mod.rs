mod compaction;
mod dedupe_role;
mod drop_role;
mod message_dedup;
mod strip_working_dir;
mod trim_context_summary;

pub use compaction::SummaryTransformer;
pub use message_dedup::ChatResponseDeduplicator;
