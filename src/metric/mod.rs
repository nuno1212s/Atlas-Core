use atlas_metrics::metrics::MetricKind;
use atlas_metrics::{MetricLevel, MetricRegistry};

/// Core frameworks will get 0XX metric ID

pub const RQ_PP_CLONE_PENDING_TIME: &str = "RQ_CLONE_PENDING_TIME";
pub const RQ_PP_CLONE_PENDING_TIME_ID: usize = 23;

pub const RQ_PP_COLLECT_PENDING_TIME: &str = "RQ_COLLECT_PENDING_TIME";
pub const RQ_PP_COLLECT_PENDING_TIME_ID: usize = 24;

pub const RQ_PP_WORKER_PROPOSER_PASSING_TIME: &str =
    "RQ_PRE_PROCESSING_WORKER_PROPOSER_PASSING_TIME";
pub const RQ_PP_WORKER_PROPOSER_PASSING_TIME_ID: usize = 21;
// Timeout metrics

pub const TIMEOUT_MESSAGE_PROCESSING: &str = "TIMEOUT_MESSAGE_PROCESSING";
pub const TIMEOUT_MESSAGE_PROCESSING_ID: usize = 30;

pub const TIMEOUT_MESSAGES_PROCESSED: &str = "TIMEOUT_MESSAGES_PROCESSED";
pub const TIMEOUT_MESSAGES_PROCESSED_ID: usize = 31;

pub fn metrics() -> Vec<MetricRegistry> {
    vec![
        (
            RQ_PP_WORKER_PROPOSER_PASSING_TIME_ID,
            RQ_PP_WORKER_PROPOSER_PASSING_TIME.to_string(),
            MetricKind::Duration,
        )
            .into(),
        (
            RQ_PP_CLONE_PENDING_TIME_ID,
            RQ_PP_CLONE_PENDING_TIME.to_string(),
            MetricKind::Duration,
            MetricLevel::Debug,
        )
            .into(),
        (
            RQ_PP_COLLECT_PENDING_TIME_ID,
            RQ_PP_COLLECT_PENDING_TIME.to_string(),
            MetricKind::Duration,
            MetricLevel::Debug,
        )
            .into(),
        (
            TIMEOUT_MESSAGE_PROCESSING_ID,
            TIMEOUT_MESSAGE_PROCESSING.to_string(),
            MetricKind::Duration,
            MetricLevel::Debug,
        )
            .into(),
        (
            TIMEOUT_MESSAGES_PROCESSED_ID,
            TIMEOUT_MESSAGES_PROCESSED.to_string(),
            MetricKind::Counter,
            MetricLevel::Debug,
        )
            .into(),
    ]
}
