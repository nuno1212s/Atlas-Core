use atlas_metrics::{MetricLevel, MetricRegistry};
use atlas_metrics::metrics::MetricKind;

/// Core frameworks will get 0XX metric ID

/// Request pre processing (010-019)

pub const RQ_PP_CLONE_PENDING_TIME: &str = "RQ_CLONE_PENDING_TIME";
pub const RQ_PP_CLONE_PENDING_TIME_ID: usize = 023;

pub const RQ_PP_COLLECT_PENDING_TIME: &str = "RQ_COLLECT_PENDING_TIME";
pub const RQ_PP_COLLECT_PENDING_TIME_ID: usize = 024;

pub const RQ_PP_WORKER_PROPOSER_PASSING_TIME: &str = "RQ_PRE_PROCESSING_WORKER_PROPOSER_PASSING_TIME";
pub const RQ_PP_WORKER_PROPOSER_PASSING_TIME_ID: usize = 021;
// Timeout metrics

pub const TIMEOUT_MESSAGE_PROCESSING: &str = "TIMEOUT_MESSAGE_PROCESSING";
pub const TIMEOUT_MESSAGE_PROCESSING_ID: usize = 030;

pub const TIMEOUT_MESSAGES_PROCESSED: &str = "TIMEOUT_MESSAGES_PROCESSED";
pub const TIMEOUT_MESSAGES_PROCESSED_ID: usize = 031;

pub fn metrics() -> Vec<MetricRegistry> {
    vec![
        (RQ_PP_WORKER_PROPOSER_PASSING_TIME_ID, RQ_PP_WORKER_PROPOSER_PASSING_TIME.to_string(), MetricKind::Duration).into(),
        (RQ_PP_CLONE_PENDING_TIME_ID, RQ_PP_CLONE_PENDING_TIME.to_string(), MetricKind::Duration, MetricLevel::Debug).into(),
        (RQ_PP_COLLECT_PENDING_TIME_ID, RQ_PP_COLLECT_PENDING_TIME.to_string(), MetricKind::Duration, MetricLevel::Debug).into(),
        (TIMEOUT_MESSAGE_PROCESSING_ID, TIMEOUT_MESSAGE_PROCESSING.to_string(), MetricKind::Duration, MetricLevel::Debug).into(),
        (TIMEOUT_MESSAGES_PROCESSED_ID, TIMEOUT_MESSAGES_PROCESSED.to_string(), MetricKind::Counter, MetricLevel::Debug).into(),
    ]
}