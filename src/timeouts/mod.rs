#![allow(dead_code)]

use std::iter;
use std::marker::PhantomData;
use std::time::{Duration, Instant};

use log::{error, warn};

use atlas_common::channel;
use atlas_common::channel::{ChannelSyncRx, ChannelSyncTx};
use atlas_common::node_id::NodeId;
use atlas_common::ordering::SeqNo;

use crate::messages::{ClientRqInfo, Message, SessionBased};
use crate::request_pre_processing::work_dividers::WDRoundRobin;
use crate::request_pre_processing::WorkPartitioner;
use crate::timeouts::worker::{TimeoutWorker, TimeoutWorkerMessage};

mod worker;

const CHANNEL_SIZE: usize = 16384;

///Contains the requests that have just been timed out
pub type Timeout = Vec<TimeoutKind>;

/// Contains a vector of requests
pub type TimedOut = Vec<RqTimeout>;

type TimeoutWorkerId = u32;

#[derive(Eq, Ord, PartialOrd, Clone, Debug)]
pub enum TimeoutKind {
    ///Relates to the timeout of a client request.
    /// Stores the client who sent it, along with the request
    /// session and request sequence number
    ClientRequestTimeout(ClientRqInfo),

    ///TODO: Maybe add a timeout for synchronizer messages?
    /// Having a timeout for STOP messages is essential for liveness
    //Sync(),

    /// As for CST messages, these messages aren't particularly ordered, they are just
    /// for each own node to know to what messages the peers are responding to.
    Cst(SeqNo),

    /// As for LTP messages, they are also not particularly ordered, similarly to the
    /// messages sent by the state transfer protocol
    LogTransfer(SeqNo),

    /// Reconfiguration message timeouts
    Reconfiguration(SeqNo),
}

#[derive(Clone, Debug)]
pub enum TimeoutPhase {
    /// The given request has timed out X times, the last of which was in Y instant
    TimedOut(usize, Instant),
}

/// A timeout for a given client request
#[derive(Clone, Debug)]
pub struct RqTimeout {
    timeout_kind: TimeoutKind,
    timeout_phase: TimeoutPhase,
}

type TimeoutMessage = MessageType;

enum MessageType {
    TimeoutRequest(RqTimeoutMessage),
    MessagesReceived(ReceivedRequest),
    ResetClientTimeouts(Duration),
    ClearClientTimeouts(Option<Vec<ClientRqInfo>>),
    ClearCstTimeouts(Option<SeqNo>),
    ClearReconfigTimeouts(Option<SeqNo>),
}

enum ReceivedRequest {
    //The node that proposed the message and all the requests contained within it
    PrePrepareRequestReceived(NodeId, Vec<ClientRqInfo>),
    //Receive a CST message relating to the following sequence number from the given
    //Node
    Cst(NodeId, SeqNo),
    // Log transfer message
    LT(NodeId, SeqNo),
    // Received a reconfiguration request response
    Reconfiguration(NodeId, SeqNo),
}

struct RqTimeoutMessage {
    timeout: Duration,
    notifications_needed: u32,
    timeout_info: Timeout,
}

#[derive(Clone)]
/// The handle to the timeouts module.
/// All messages destined to the timeouts should be passed through here
pub struct Timeouts {
    handle: ChannelSyncTx<TimeoutMessage>,
}

impl Timeouts {
    ///Initialize the timeouts thread and return a handle to it
    /// This handle can then be used everywhere timeouts are needed.
    pub fn new<RQ: SessionBased + 'static>(
        node_id: NodeId,
        default_timeout: Duration,
        loopback_channel: ChannelSyncTx<Message>,
    ) -> Self {
        launch_orchestrator_thread::<WDRoundRobin, RQ>(
            2,
            node_id,
            default_timeout,
            loopback_channel,
        )
    }

    /// Start a timeout request on the list of digests that have been provided
    pub fn timeout_client_requests(&self, timeout: Duration, requests: Vec<ClientRqInfo>) {
        let requests: Vec<TimeoutKind> = requests
            .into_iter()
            .map(TimeoutKind::ClientRequestTimeout)
            .collect();

        let res = self
            .handle
            .try_send_return(TimeoutMessage::TimeoutRequest(RqTimeoutMessage {
                timeout,
                // we choose 1 here because we only need to receive one valid pre prepare containing
                // this request for it to be considered valid
                notifications_needed: 1,
                timeout_info: requests,
            }));

        if let Err(err) = res {
            warn!(
                "Discarding Client Request timeout message as queue is already full {:?}",
                err
            )
        }
    }

    /// Notify that a pre prepare with the following requests has been received and we must therefore
    /// Disable any timeouts pertaining to the received requests
    pub fn received_pre_prepare(&self, from: NodeId, recvd_rqs: Vec<ClientRqInfo>) {
        let res = self
            .handle
            .try_send_return(TimeoutMessage::MessagesReceived(
                ReceivedRequest::PrePrepareRequestReceived(from, recvd_rqs),
            ));

        if let Err(err) = res {
            warn!(
                "Discarding pre prepare timeout message as queue is already full {:?}",
                err
            )
        }
    }

    /// Set the timeout phase of all timeouts to the initial state (0 timeouts) and re call all of the timeouts
    pub fn reset_all_client_rq_timeouts(&self, duration: Duration) {
        self.handle
            .send_return(TimeoutMessage::ResetClientTimeouts(duration))
            .expect("Failed to contact timeout thread")
    }

    /// Cancel timeouts of player requests.
    /// This accepts an option. If this Option is None, then the
    /// timeouts for all client requests are going to be disabled
    pub fn cancel_client_rq_timeouts(&self, requests_to_clear: Option<Vec<ClientRqInfo>>) {
        self.handle
            .send_return(TimeoutMessage::ClearClientTimeouts(requests_to_clear))
            .expect("Failed to contact timeout thread")
    }

    /// Timeout a CST request
    pub fn timeout_cst_request(&self, timeout: Duration, requests_needed: u32, seq_no: SeqNo) {
        self.handle
            .send_return(TimeoutMessage::TimeoutRequest(RqTimeoutMessage {
                timeout,
                notifications_needed: requests_needed,
                timeout_info: vec![TimeoutKind::Cst(seq_no)],
            }))
            .expect("Failed to contact timeout thread");
    }

    pub fn timeout_lt_request(&self, timeout: Duration, requests_needed: u32, seq_no: SeqNo) {
        self.handle
            .send_return(TimeoutMessage::TimeoutRequest(RqTimeoutMessage {
                timeout,
                notifications_needed: requests_needed,
                timeout_info: vec![TimeoutKind::LogTransfer(seq_no)],
            }))
            .expect("Failed to contact timeout thread");
    }

    pub fn timeout_reconfig_request(&self, timeout: Duration, requests_needed: u32, seq_no: SeqNo) {
        self.handle
            .send_return(TimeoutMessage::TimeoutRequest(RqTimeoutMessage {
                timeout,
                notifications_needed: requests_needed,
                timeout_info: vec![TimeoutKind::Reconfiguration(seq_no)],
            }))
            .expect("Failed to contact timeout thread");
    }

    /// Handle having received a cst request
    pub fn received_cst_request(&self, from: NodeId, seq_no: SeqNo) {
        self.handle
            .send_return(TimeoutMessage::MessagesReceived(ReceivedRequest::Cst(
                from, seq_no,
            )))
            .expect("Failed to contact timeout thread");
    }

    /// Handle having received a cst request
    pub fn received_log_request(&self, from: NodeId, seq_no: SeqNo) {
        self.handle
            .send_return(TimeoutMessage::MessagesReceived(ReceivedRequest::LT(
                from, seq_no,
            )))
            .expect("Failed to contact timeout thread");
    }

    pub fn received_reconfig_request(&self, from: NodeId, seq_no: SeqNo) {
        self.handle
            .send_return(TimeoutMessage::MessagesReceived(
                ReceivedRequest::Reconfiguration(from, seq_no),
            ))
            .expect("Failed to contact timeout thread");
    }

    /// Cancel timeouts of CST messages.
    /// This accepts an option. If this Option is None, then the
    /// timeouts for all CST requests are going to be disabled.
    pub fn cancel_cst_timeout(&self, seq_no: Option<SeqNo>) {
        self.handle
            .send_return(TimeoutMessage::ClearCstTimeouts(seq_no))
            .expect("Failed to contact timeout thread");
    }

    /// Cancel timeouts of reconfig messages
    pub fn cancel_reconfig_timeout(&self, seq_no: Option<SeqNo>) {
        self.handle
            .send_return(TimeoutMessage::ClearReconfigTimeouts(seq_no))
            .expect("Failed to contact timeout thread")
    }
}

struct TimeoutOrchestrator<WP, RQ> {
    worker_count: u32,

    work_rx: ChannelSyncRx<TimeoutMessage>,

    worker_channel: Vec<ChannelSyncTx<TimeoutWorkerMessage>>,

    work_partition: PhantomData<fn() -> (WP, RQ)>,
}

impl<WP, RQ> TimeoutOrchestrator<WP, RQ> {
    fn new(
        worker_count: u32,
        work_rx: ChannelSyncRx<TimeoutMessage>,
        workers: Vec<ChannelSyncTx<TimeoutWorkerMessage>>,
    ) -> Self {
        Self {
            worker_count,
            work_rx,
            worker_channel: workers,
            work_partition: Default::default(),
        }
    }

    fn run(self)
        where
            RQ: SessionBased,
            WP: WorkPartitioner<RQ>,
    {
        loop {
            let message = match self.work_rx.recv() {
                Ok(message) => message,
                Err(error) => {
                    error!("Timeout orchestrator failed to receive message {:?}", error);

                    break;
                }
            };

            match message {
                TimeoutMessage::TimeoutRequest(request) => {
                    self.handle_timeout_request(request);
                }
                TimeoutMessage::MessagesReceived(messages) => {
                    self.handle_messages_received(messages);
                }
                TimeoutMessage::ResetClientTimeouts(duration) => {
                    for work_channel in &self.worker_channel {
                        work_channel
                            .send_return(TimeoutWorkerMessage::ResetClientTimeouts(duration))
                            .expect("Failed to send worker message")
                    }
                }
                TimeoutMessage::ClearClientTimeouts(clear_timeouts) => {
                    self.handle_clear_client_timeouts(clear_timeouts);
                }
                TimeoutMessage::ClearCstTimeouts(seq) => {
                    self.worker_channel[0]
                        .send_return(TimeoutWorkerMessage::ClearCstTimeouts(seq))
                        .expect("Failed to contact worker");
                }
                TimeoutMessage::ClearReconfigTimeouts(seq) => self.worker_channel[0]
                    .send_return(TimeoutWorkerMessage::ClearReconfigTimeouts(seq))
                    .expect("Failed to contact worker"),
            }
        }
    }

    fn handle_timeout_request(&self, request: RqTimeoutMessage)
        where
            RQ: SessionBased,
            WP: WorkPartitioner<RQ>,
    {
        let RqTimeoutMessage {
            timeout,
            notifications_needed,
            timeout_info,
        } = request;

        let mut separated_vecs: Vec<Vec<TimeoutKind>> =
            self.init_worker_separated_vec(|| Vec::with_capacity(timeout_info.len()));

        for timeout in timeout_info {
            match &timeout {
                TimeoutKind::ClientRequestTimeout(client_rq) => {
                    let worker =
                        WP::get_worker_for_processed(client_rq, self.worker_count as usize);

                    separated_vecs[worker].push(timeout);
                }
                _ => {
                    separated_vecs[0].push(timeout);
                }
            }
        }

        for (work, worker) in iter::zip(separated_vecs, &self.worker_channel) {
            let work_msg = TimeoutWorkerMessage::TimeoutRequest(RqTimeoutMessage {
                timeout,
                notifications_needed,
                timeout_info: work,
            });

            worker
                .send_return(work_msg)
                .expect("Failed to send worker message");
        }
    }

    fn handle_messages_received(&self, messages: ReceivedRequest)
        where
            RQ: SessionBased,
            WP: WorkPartitioner<RQ>,
    {
        match messages {
            ReceivedRequest::PrePrepareRequestReceived(sender, messages) => {
                let mut separated_vecs: Vec<Vec<ClientRqInfo>> =
                    self.init_worker_separated_vec(|| Vec::with_capacity(messages.len()));

                for recvd_rq in messages {
                    let worker =
                        WP::get_worker_for_processed(&recvd_rq, self.worker_count as usize);

                    separated_vecs[worker].push(recvd_rq);
                }

                for (work, worker) in iter::zip(separated_vecs, &self.worker_channel) {
                    let work_msg = TimeoutWorkerMessage::MessagesReceived(
                        ReceivedRequest::PrePrepareRequestReceived(sender, work),
                    );

                    worker
                        .send_return(work_msg)
                        .expect("Failed to send worker message");
                }
            }
            ReceivedRequest::Cst(sender, seq) => {
                self.worker_channel[0]
                    .send_return(TimeoutWorkerMessage::MessagesReceived(
                        ReceivedRequest::Cst(sender, seq),
                    ))
                    .expect("Failed to send worker message");
            }
            ReceivedRequest::LT(sender, seq) => {
                self.worker_channel[0]
                    .send_return(TimeoutWorkerMessage::MessagesReceived(ReceivedRequest::LT(
                        sender, seq,
                    )))
                    .expect("Failed to send worker message");
            }
            ReceivedRequest::Reconfiguration(sender, seq) => {
                self.worker_channel[0]
                    .send_return(TimeoutWorkerMessage::MessagesReceived(
                        ReceivedRequest::Reconfiguration(sender, seq),
                    ))
                    .expect("Failed to send worker message");
            }
        }
    }

    fn handle_clear_client_timeouts(&self, clear_timeouts: Option<Vec<ClientRqInfo>>)
        where
            RQ: SessionBased,
            WP: WorkPartitioner<RQ>,
    {
        let mut separated_vecs = if clear_timeouts.is_some() {
            let vec_length = clear_timeouts.as_ref().map(|t| t.len()).unwrap();

            self.init_worker_separated_vec(|| Some(Vec::with_capacity(vec_length)))
        } else {
            iter::repeat(None)
                .take(self.worker_count as usize)
                .collect()
        };

        if let Some(timeouts) = clear_timeouts {
            for rq in timeouts {
                let worker = WP::get_worker_for_processed(&rq, self.worker_count as usize);

                separated_vecs[worker].as_mut().unwrap().push(rq);
            }
        }

        for (work, worker) in iter::zip(separated_vecs, &self.worker_channel) {
            worker
                .send_return(TimeoutWorkerMessage::ClearClientTimeouts(work))
                .expect("Failed to send worker message");
        }
    }

    fn init_worker_separated_vec<T, F>(&self, capacity: F) -> Vec<T>
        where
            F: FnMut() -> T,
    {
        iter::repeat_with(capacity)
            .take(self.worker_count as usize)
            .collect()
    }
}

impl PartialEq for TimeoutKind {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::ClientRequestTimeout(client_1), Self::ClientRequestTimeout(client_2)) => {
                client_1 == client_2
            }
            (Self::Cst(seq_no_1), Self::Cst(seq_no_2)) => seq_no_1 == seq_no_2,
            (Self::LogTransfer(seq_no_1), Self::LogTransfer(seq_no_2)) => seq_no_1 == seq_no_2,
            (_, _) => false,
        }
    }
}

impl RqTimeout {
    pub fn timeout_kind(&self) -> &TimeoutKind {
        &self.timeout_kind
    }

    pub fn into_timeout_kind(self) -> TimeoutKind {
        self.timeout_kind
    }

    pub fn timeout_phase(&self) -> &TimeoutPhase {
        &self.timeout_phase
    }
}

impl TimeoutPhase {
    #![allow(unreachable_patterns)]

    fn timeout_count(&self) -> usize {
        match self {
            Self::TimedOut(times, _) => *times,
            _ => 0,
        }
    }

    fn timeout_instant(&self) -> Instant {
        match self {
            Self::TimedOut(_, instant) => *instant,
            _ => unreachable!(),
        }
    }
}

fn launch_orchestrator_thread<WP, RQ>(
    worker_count: u32,
    node_id: NodeId,
    timeout_dur: Duration,
    loopback: ChannelSyncTx<Message>,
) -> Timeouts
    where
        RQ: SessionBased + 'static,
        WP: WorkPartitioner<RQ> + 'static,
{
    let (tx, rx) = channel::new_bounded_sync(CHANNEL_SIZE, Some("Timeout Orchestrator Thread"));

    let mut workers = Vec::with_capacity(worker_count as usize);

    for i in 0..worker_count {
        let worker = TimeoutWorker::start_worker(i, node_id, timeout_dur, loopback.clone());

        workers.push(worker);
    }

    let orchestrator: TimeoutOrchestrator<WP, RQ> =
        TimeoutOrchestrator::new(worker_count, rx, workers);

    std::thread::Builder::new()
        .name("Timeout-Orchestrator".to_string())
        .spawn(move || orchestrator.run())
        .expect("Failed to launch timeout orchestrator thread");

    Timeouts { handle: tx }
}
