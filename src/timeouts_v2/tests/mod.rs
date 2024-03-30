#![cfg(test)]

mod timeouts_test {
    use std::sync::Arc;
    use std::time::Duration;
    use lazy_static::lazy_static;

    use atlas_common::channel::{new_bounded_sync, ChannelSyncRx, ChannelSyncTx};
    use atlas_common::node_id::NodeId;
    use atlas_common::ordering::SeqNo;

    use crate::timeouts_v2::{
        initialize_timeouts, Timeout, TimeoutID, TimeoutIdentification, TimeoutWorkerResponder,
        TimeoutsHandle,
    };

    const OUR_ID: NodeId = NodeId(0);

    lazy_static! {static ref MOD_NAME: Arc<str> = Arc::from("TestMod"); }

    #[derive(Clone)]
    struct TimeoutTx {
        tx: ChannelSyncTx<Vec<Timeout>>,
    }

    impl TimeoutWorkerResponder for TimeoutTx {
        fn report_timeouts(&self, timeouts: Vec<Timeout>) -> atlas_common::error::Result<()> {
            self.tx.send(timeouts)
        }
    }


    fn init_timeout_rx() -> (TimeoutTx, ChannelSyncRx<Vec<Timeout>>) {
        let (tx, rx) = new_bounded_sync(128, Some("TimeoutRx"));
        (TimeoutTx { tx }, rx)
    }

    fn setup_timeouts() -> (TimeoutsHandle, ChannelSyncRx<Vec<Timeout>>) {
        let (tx, rx) = init_timeout_rx();

        let handle = initialize_timeouts(OUR_ID, 1, 10, tx);

        (handle, rx)
    }

    #[test]
    fn test_timeout_session_based() {
        let (handle, timeout_rx) = setup_timeouts();

        handle
            .request_timeout(
                TimeoutIdentification {
                    mod_id: MOD_NAME.clone(),
                    timeout_id: TimeoutID::SessionBased {
                        session: SeqNo::ZERO,
                        seq_no: SeqNo::ZERO,
                        from: OUR_ID,
                    },
                },
                None,
                Duration::from_secs(1),
                1,
                false,
            )
            .unwrap();

        let timeouts = timeout_rx.recv().unwrap();

        assert_eq!(timeouts.len(), 1);

        assert!(matches!(
            timeouts[0].id.timeout_id,
            TimeoutID::SessionBased {
                session: SeqNo::ZERO,
                seq_no: SeqNo::ZERO,
                from: OUR_ID
            }
        ));
    }
    
    #[test]
    fn test_timeout_ack() {
        todo!()
    }
}
