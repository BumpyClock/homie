use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::extract::ws::Message as WsMessage;
use tokio::sync::mpsc::{self, error::TrySendError};
use uuid::Uuid;

use crate::debug_bytes::{contains_subseq, fmt_bytes, terminal_debug_enabled_for};
use crate::outbound::OutboundMessage;
use homie_protocol::{BinaryFrame, StreamType};

use super::history::{HistoryBuffer, HISTORY_CHUNK_BYTES};

pub(super) async fn forward_pty_output(
    session_id: Uuid,
    mut output_rx: mpsc::Receiver<Vec<u8>>,
    subscribers: Arc<Mutex<HashMap<Uuid, mpsc::Sender<OutboundMessage>>>>,
    history: Arc<Mutex<HistoryBuffer>>,
) {
    while let Some(data) = output_rx.recv().await {
        if terminal_debug_enabled_for(session_id) {
            let has_dsr = contains_subseq(&data, b"\x1b[6n") || contains_subseq(&data, b"[6n");
            tracing::info!(
                session = %session_id,
                dsr = has_dsr,
                msg = %fmt_bytes(&data, 80),
                "terminal pty out"
            );
        }
        if let Ok(mut buffer) = history.lock() {
            buffer.push(&data);
        }
        let frame = BinaryFrame {
            session_id,
            stream: StreamType::Stdout,
            payload: data,
        };
        let encoded = frame.encode();
        let mut guard = subscribers.lock().unwrap();
        guard.retain(|_, tx| {
            match tx.try_send(OutboundMessage::raw(WsMessage::Binary(
                encoded.clone().into(),
            ))) {
                Ok(()) => true,
                Err(TrySendError::Full(_)) => true,
                Err(TrySendError::Closed(_)) => false,
            }
        });
    }
}

pub(super) fn replay_history(
    session_id: Uuid,
    snapshot: Vec<u8>,
    max_bytes: usize,
    outbound_tx: mpsc::Sender<OutboundMessage>,
) {
    if snapshot.is_empty() {
        return;
    }
    let slice = if max_bytes > 0 && snapshot.len() > max_bytes {
        snapshot[snapshot.len() - max_bytes..].to_vec()
    } else {
        snapshot
    };
    tokio::spawn(async move {
        for chunk in slice.chunks(HISTORY_CHUNK_BYTES) {
            if terminal_debug_enabled_for(session_id) {
                tracing::info!(
                    session = %session_id,
                    msg = %fmt_bytes(chunk, 80),
                    "terminal replay chunk"
                );
            }
            let frame = BinaryFrame {
                session_id,
                stream: StreamType::Stdout,
                payload: chunk.to_vec(),
            };
            match outbound_tx.try_send(OutboundMessage::raw(WsMessage::Binary(
                frame.encode().into(),
            ))) {
                Ok(()) | Err(TrySendError::Full(_)) => {}
                Err(TrySendError::Closed(_)) => break,
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use homie_protocol::BinaryFrame;
    use tokio::sync::mpsc;
    use uuid::Uuid;

    use super::*;

    #[tokio::test]
    async fn fanout_drops_full_subscribers_removes_closed_and_records_history() {
        let session_id = Uuid::new_v4();
        let open_id = Uuid::new_v4();
        let full_id = Uuid::new_v4();
        let closed_id = Uuid::new_v4();

        let (open_tx, mut open_rx) = mpsc::channel(1);
        let (full_tx, mut full_rx) = mpsc::channel(1);
        full_tx
            .try_send(OutboundMessage::event("preloaded", None))
            .unwrap();
        let (closed_tx, closed_rx) = mpsc::channel(1);
        drop(closed_rx);

        let subscribers = Arc::new(Mutex::new(HashMap::from([
            (open_id, open_tx),
            (full_id, full_tx),
            (closed_id, closed_tx),
        ])));
        let history = Arc::new(Mutex::new(HistoryBuffer::new(1024)));
        let (output_tx, output_rx) = mpsc::channel(1);

        let task = tokio::spawn(forward_pty_output(
            session_id,
            output_rx,
            subscribers.clone(),
            history.clone(),
        ));

        output_tx.send(b"abc".to_vec()).await.unwrap();
        drop(output_tx);
        task.await.unwrap();

        assert_eq!(history.lock().unwrap().snapshot(), b"abc");

        let open_msg = open_rx.recv().await.unwrap();
        let OutboundMessage::Raw(WsMessage::Binary(bytes)) = open_msg else {
            panic!("expected binary terminal frame");
        };
        let frame = BinaryFrame::decode(&bytes).unwrap();
        assert_eq!(frame.session_id, session_id);
        assert_eq!(frame.payload, b"abc");

        assert!(matches!(
            full_rx.try_recv().unwrap(),
            OutboundMessage::Event { .. }
        ));
        assert!(full_rx.try_recv().is_err());

        let guard = subscribers.lock().unwrap();
        assert!(guard.contains_key(&open_id));
        assert!(guard.contains_key(&full_id));
        assert!(!guard.contains_key(&closed_id));
    }
}
