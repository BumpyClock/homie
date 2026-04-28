use axum::extract::ws::{CloseFrame, Message, WebSocket};
use futures::{stream::SplitSink, stream::SplitStream, SinkExt, StreamExt};
use homie_protocol::{encode_message, Message as ProtoMessage};
use tokio::sync::{broadcast, mpsc};

use crate::agent::ChatService;
use crate::authz::Scope;
use crate::connection::routing::handle_text_message;
use crate::connection::types::MessageLoopParams;
use crate::debug_bytes::{fmt_bytes, terminal_debug_enabled_for};
use crate::notifications::NotificationsService;
use crate::outbound::OutboundMessage;
use crate::pairing::PairingService;
use crate::presence::PresenceService;
use crate::router::{MessageRouter, SubscriptionManager};
use crate::terminal::TerminalService;
use crate::{CronService, JobsService};

pub(super) async fn run_message_loop(
    sink: &mut SplitSink<WebSocket, Message>,
    stream: &mut SplitStream<WebSocket>,
    params: MessageLoopParams,
) {
    let MessageLoopParams {
        conn_id,
        heartbeat_interval,
        idle_timeout,
        authz,
        store,
        nodes,
        terminal_registry,
        event_tx,
        cron_runner,
        homie_config,
        exec_policy,
        roci,
        pairing_default_ttl_secs,
        pairing_retention_secs,
        tool_channel,
    } = params;
    let mut idle_deadline = tokio::time::Instant::now() + idle_timeout;
    let mut heartbeat = tokio::time::interval(heartbeat_interval);
    heartbeat.tick().await; // consume immediate first tick

    // Outbound channel: services push PTY output + events here.
    // Bounded for backpressure — services use try_send to avoid blocking.
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<OutboundMessage>(256);

    // Build the router with services.
    let mut router = MessageRouter::new();
    router.register(Box::new(TerminalService::new(
        conn_id,
        terminal_registry,
        outbound_tx.clone(),
        event_tx.clone(),
    )));
    let (chat_service, agent_service) = ChatService::new_shared_with_channel(
        outbound_tx.clone(),
        store.clone(),
        homie_config,
        exec_policy,
        roci,
        tool_channel,
    );
    router.register(Box::new(chat_service));
    router.register(Box::new(agent_service));
    router.register(Box::new(PresenceService::new(nodes)));
    router.register(Box::new(JobsService::new(store.clone())));
    router.register(Box::new(PairingService::new(
        store.clone(),
        pairing_default_ttl_secs,
        pairing_retention_secs,
    )));
    router.register(Box::new(CronService::new(
        store.clone(),
        cron_runner.clone(),
    )));
    router.register(Box::new(NotificationsService::new(
        store.clone(),
        outbound_tx.clone(),
    )));

    // Per-connection subscription manager.
    let mut subscriptions = SubscriptionManager::new();

    let mut event_rx = event_tx.subscribe();

    loop {
        tokio::select! {
            // Incoming WS message.
            msg = stream.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        idle_deadline = tokio::time::Instant::now() + idle_timeout;
                        handle_text_message(
                            sink,
                            &text,
                            authz,
                            &mut router,
                            &mut subscriptions,
                        )
                        .await;
                    }
                    Some(Ok(Message::Binary(data))) => {
                        idle_deadline = tokio::time::Instant::now() + idle_timeout;
                        if authz.allows(Scope::TerminalWrite) {
                            let decoded = homie_protocol::BinaryFrame::decode(&data);
                            match decoded {
                                Ok(frame) => {
                                    if terminal_debug_enabled_for(frame.session_id) {
                                        tracing::info!(
                                            session = %frame.session_id,
                                            stream = ?frame.stream,
                                            msg = %fmt_bytes(&frame.payload, 80),
                                            "terminal ws in binary"
                                        );
                                    }
                                    router.route_binary(&frame);
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        err = %e,
                                        msg = %fmt_bytes(&data, 64),
                                        "invalid binary frame"
                                    );
                                }
                            }
                        } else {
                            tracing::debug!("unauthorized binary frame ignored");
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        idle_deadline = tokio::time::Instant::now() + idle_timeout;
                        let _ = sink.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Pong(_))) => {
                        idle_deadline = tokio::time::Instant::now() + idle_timeout;
                    }
                    Some(Ok(Message::Close(frame))) => {
                        if let Some(frame) = frame {
                            tracing::info!(code = %frame.code, reason = %frame.reason, "ws close");
                        } else {
                            tracing::info!("ws close");
                        }
                        break;
                    }
                    None => {
                        tracing::info!("ws stream ended");
                        break;
                    }
                    Some(Err(e)) => {
                        tracing::warn!("ws error: {e}");
                        break;
                    }
                }
            }
            // Outbound messages from services (PTY output frames).
            msg = outbound_rx.recv() => {
                match msg {
                    Some(OutboundMessage::Raw(m)) => {
                        if let Message::Binary(data) = &m {
                            match homie_protocol::BinaryFrame::decode(data) {
                                Ok(frame) => {
                                    if terminal_debug_enabled_for(frame.session_id) {
                                        tracing::info!(
                                            session = %frame.session_id,
                                            stream = ?frame.stream,
                                            msg = %fmt_bytes(&frame.payload, 80),
                                            "terminal ws out binary"
                                        );
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        err = %e,
                                        msg = %fmt_bytes(data, 64),
                                        "invalid outbound binary frame"
                                    );
                                }
                            }
                        }
                        let _ = sink.send(m).await;
                    }
                    Some(OutboundMessage::Event { topic, params }) => {
                        if subscriptions.matches(&topic) {
                            let evt = ProtoMessage::Event(homie_protocol::Event {
                                topic,
                                params,
                            });
                            if let Ok(json) = encode_message(&evt) {
                                let _ = sink.send(Message::Text(json.into())).await;
                            }
                        }
                    }
                    None => break,
                }
            }
            // Heartbeat ping.
            _ = heartbeat.tick() => {
                if sink.send(Message::Ping(vec![].into())).await.is_err() {
                    break;
                }
            }
            // Idle timeout.
            _ = tokio::time::sleep_until(idle_deadline) => {
                tracing::info!("idle timeout");
                let _ = sink
                    .send(Message::Close(Some(CloseFrame {
                        code: 4000,
                        reason: "idle timeout".into(),
                    })))
                    .await;
                break;
            }
            // Broadcast events (filtered by subscriptions).
            evt = event_rx.recv() => {
                match evt {
                    Ok(reap_event) => {
                        tracing::info!(topic = %reap_event.topic, "broadcast event");
                        if subscriptions.matches(&reap_event.topic) {
                            let evt = ProtoMessage::Event(homie_protocol::Event {
                                topic: reap_event.topic,
                                params: reap_event.params,
                            });
                            if let Ok(json) = encode_message(&evt) {
                                let _ = sink.send(Message::Text(json.into())).await;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        tracing::warn!("event receiver lagged");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        }
    }

    // Connection closing — clean up all services.
    router.shutdown_all();
}
