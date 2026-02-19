use std::collections::HashMap;
use std::sync::Arc;

use roci::agent_loop::{
    ApprovalDecision, LoopRunner, RunEvent, RunEventPayload, RunHooks, RunLifecycle, RunRequest,
    Runner,
};
use roci::types::ModelMessage;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use super::events::{
    approval_cache_key, approval_command_argv, emit_approval_required, emit_diff_updated,
    emit_error, emit_item_completed, emit_message_delta, emit_plan_updated, emit_reasoning_delta,
    emit_tool_item_completed, emit_tool_item_started, emit_turn_completed, ToolEventContext,
    ToolItemCompletedData, ToolItemStartedData,
};
use super::persistence::{
    persist_roci_raw_event, persist_thread_snapshot, PersistedThreadSnapshot,
};
use super::state::{
    model_tool_call_message, upsert_tool_item_completed, upsert_tool_item_started, PendingRun,
    RociRunState, ToolCallInfo,
};

pub(super) async fn start_run_inner(
    backend: super::RociBackend,
    pending: PendingRun,
) -> Result<(), String> {
    let run_id = Uuid::parse_str(&pending.turn_id).unwrap_or_else(|_| Uuid::new_v4());
    if super::debug_enabled() {
        tracing::debug!(
            chat_id = %pending.chat_id,
            thread_id = %pending.thread_id,
            turn_id = %pending.turn_id,
            run_id = %run_id,
            model = %pending.model.to_string(),
            "roci start_run"
        );
    }

    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<RunEvent>();
    let event_sink = Arc::new(move |event: RunEvent| {
        let _ = event_tx.send(event);
    });

    let state = backend.state.clone();
    let exec_policy = backend.exec_policy.clone();
    let thread_id_for_cache = pending.thread_id.clone();
    let approval_handler: roci::agent_loop::ApprovalHandler = Arc::new(move |request| {
        let state = state.clone();
        let exec_policy = exec_policy.clone();
        let thread_id = thread_id_for_cache.clone();
        Box::pin(async move {
            if request.kind == roci::agent_loop::ApprovalKind::CommandExecution {
                if let Some(argv) = approval_command_argv(&request.payload) {
                    if exec_policy.is_allowed(&argv) {
                        if super::debug_enabled() {
                            tracing::debug!(argv = ?argv, "roci execpolicy auto-approve");
                        }
                        return ApprovalDecision::Accept;
                    }
                }
            }
            let cache_key = approval_cache_key(&request);
            if let Some(key) = cache_key.as_ref() {
                let cached = {
                    let guard = state.lock().await;
                    guard
                        .approval_cache
                        .get(&thread_id)
                        .map(|set| set.contains(key))
                        .unwrap_or(false)
                };
                if cached {
                    return ApprovalDecision::Accept;
                }
            }
            let (tx, rx) = oneshot::channel();
            {
                let mut guard = state.lock().await;
                guard.approvals.insert(request.id.clone(), tx);
            }
            let decision = rx.await.unwrap_or(ApprovalDecision::Decline);
            let mut guard = state.lock().await;
            guard.approvals.remove(&request.id);
            if matches!(decision, ApprovalDecision::AcceptForSession) {
                if let Some(key) = cache_key {
                    guard
                        .approval_cache
                        .entry(thread_id)
                        .or_default()
                        .insert(key);
                }
            }
            decision
        })
    });

    let mut run_request = RunRequest::new(pending.model, pending.messages);
    run_request.run_id = run_id;
    run_request.settings = pending.settings;
    run_request.tools = backend.tools.clone();
    run_request.approval_policy = pending.approval_policy;
    run_request.event_sink = Some(event_sink);
    run_request.approval_handler = Some(approval_handler);
    run_request.hooks = RunHooks {
        compaction: Some(Arc::new(compact_messages)),
        tool_result_persist: Some(Arc::new(trim_tool_result)),
    };

    let runner = LoopRunner::new(pending.config);
    let handle = runner
        .start(run_request)
        .await
        .map_err(|e| format!("run start failed: {e}"))?;

    {
        let mut state = backend.state.lock().await;
        state.runs.insert(
            pending.turn_id.clone(),
            RociRunState {
                thread_id: pending.thread_id.clone(),
                handle: Some(handle),
            },
        );
    }

    let outbound = backend.outbound_tx.clone();
    let store = backend.store.clone();
    let state = backend.state.clone();
    let thread_id = pending.thread_id.clone();
    let turn_id_clone = pending.turn_id.clone();
    let chat_id = pending.chat_id.clone();
    let assistant_item_id_clone = pending.assistant_item_id.clone();
    let collaboration_mode = pending.collaboration_mode.clone();
    let raw_events_enabled = backend.raw_events_enabled;
    let backend_for_task = backend.clone();

    tokio::spawn(async move {
        let mut assistant_text = String::new();
        let mut tool_calls: HashMap<String, ToolCallInfo> = HashMap::new();
        while let Some(event) = event_rx.recv().await {
            match event.payload {
                RunEventPayload::AssistantDelta { text } => {
                    if !text.is_empty() {
                        if super::debug_enabled() {
                            tracing::debug!(
                                %chat_id,
                                %thread_id,
                                %turn_id_clone,
                                delta_len = text.len(),
                                "roci assistant delta"
                            );
                        }
                        assistant_text.push_str(&text);
                        emit_message_delta(
                            &outbound,
                            &store,
                            &chat_id,
                            &thread_id,
                            &turn_id_clone,
                            &assistant_item_id_clone,
                            &text,
                        );
                        if raw_events_enabled {
                            persist_roci_raw_event(
                                &store,
                                &turn_id_clone,
                                &thread_id,
                                "item/agentMessage/delta",
                                serde_json::json!({
                                    "threadId": thread_id,
                                    "turnId": turn_id_clone,
                                    "itemId": assistant_item_id_clone,
                                    "delta": text,
                                }),
                            );
                        }
                    }
                }
                RunEventPayload::ReasoningDelta { text } => {
                    if !text.is_empty() {
                        if super::debug_enabled() {
                            tracing::debug!(
                                %chat_id,
                                %thread_id,
                                %turn_id_clone,
                                delta_len = text.len(),
                                "roci reasoning delta"
                            );
                        }
                        emit_reasoning_delta(
                            &outbound,
                            &store,
                            &chat_id,
                            &thread_id,
                            &turn_id_clone,
                            &assistant_item_id_clone,
                            &text,
                        );
                    }
                }
                RunEventPayload::ToolCallStarted { call } => {
                    if super::debug_enabled() {
                        tracing::debug!(
                            %chat_id,
                            %thread_id,
                            %turn_id_clone,
                            tool_call_id = %call.id,
                            tool = %call.name,
                            "roci tool call started"
                        );
                    }
                    {
                        let mut guard = state.lock().await;
                        if let Some(thread) = guard.threads.get_mut(&thread_id) {
                            if let Some(turn) = thread
                                .thread
                                .turns
                                .iter_mut()
                                .find(|turn| turn.id == turn_id_clone)
                            {
                                upsert_tool_item_started(
                                    turn,
                                    &call.id,
                                    &call.name,
                                    call.arguments.clone(),
                                );
                            }
                            thread.messages.push(model_tool_call_message(&call));
                            thread.thread.updated_at = super::now_unix();
                        }
                    }
                    backend_for_task.persist_thread_state(&thread_id).await;
                    if raw_events_enabled {
                        persist_roci_raw_event(
                            &store,
                            &turn_id_clone,
                            &thread_id,
                            "item/started",
                            serde_json::json!({
                                "threadId": thread_id,
                                "turnId": turn_id_clone,
                                "item": {
                                    "id": call.id.clone(),
                                    "type": "mcpToolCall",
                                    "tool": call.name.clone(),
                                    "status": "running",
                                    "input": call.arguments.clone(),
                                },
                            }),
                        );
                    }
                    tool_calls.insert(
                        call.id.clone(),
                        ToolCallInfo {
                            name: call.name.clone(),
                            input: call.arguments.clone(),
                        },
                    );
                    emit_tool_item_started(
                        ToolEventContext::new(
                            &outbound,
                            &store,
                            &chat_id,
                            &thread_id,
                            &turn_id_clone,
                        ),
                        ToolItemStartedData::new(&call.id, &call.name, &call.arguments),
                    );
                }
                RunEventPayload::ToolResult { result } => {
                    if super::debug_enabled() {
                        tracing::debug!(
                            %chat_id,
                            %thread_id,
                            %turn_id_clone,
                            tool_call_id = %result.tool_call_id,
                            is_error = result.is_error,
                            result = %result.result,
                            "roci tool result"
                        );
                    }
                    let info =
                        tool_calls
                            .remove(&result.tool_call_id)
                            .unwrap_or_else(|| ToolCallInfo {
                                name: "tool".to_string(),
                                input: serde_json::Value::Null,
                            });
                    {
                        let mut guard = state.lock().await;
                        if let Some(thread) = guard.threads.get_mut(&thread_id) {
                            if let Some(turn) = thread
                                .thread
                                .turns
                                .iter_mut()
                                .find(|turn| turn.id == turn_id_clone)
                            {
                                upsert_tool_item_completed(
                                    turn,
                                    &result.tool_call_id,
                                    &info.name,
                                    info.input.clone(),
                                    result.result.clone(),
                                    result.is_error,
                                );
                            }
                            thread.messages.push(ModelMessage::tool_result(
                                result.tool_call_id.clone(),
                                result.result.clone(),
                                result.is_error,
                            ));
                            thread.thread.updated_at = super::now_unix();
                        }
                    }
                    backend_for_task.persist_thread_state(&thread_id).await;
                    if raw_events_enabled {
                        let status = if result.is_error {
                            "failed"
                        } else {
                            "completed"
                        };
                        persist_roci_raw_event(
                            &store,
                            &turn_id_clone,
                            &thread_id,
                            "item/completed",
                            serde_json::json!({
                                "threadId": thread_id,
                                "turnId": turn_id_clone,
                                "item": {
                                    "id": result.tool_call_id.clone(),
                                    "type": "mcpToolCall",
                                    "tool": info.name.clone(),
                                    "status": status,
                                    "input": info.input.clone(),
                                    "result": result.result.clone(),
                                    "error": result.is_error,
                                },
                            }),
                        );
                    }
                    if let Some(process_id) =
                        exec_process_id_from_result(&info.name, &result.result)
                    {
                        backend_for_task
                            .record_tool_process(&thread_id, &turn_id_clone, process_id)
                            .await;
                    }
                    emit_tool_item_completed(
                        ToolEventContext::new(
                            &outbound,
                            &store,
                            &chat_id,
                            &thread_id,
                            &turn_id_clone,
                        ),
                        ToolItemCompletedData::new(
                            &result.tool_call_id,
                            &info.name,
                            &info.input,
                            &result.result,
                            result.is_error,
                        ),
                    );
                    if info.name == "apply_patch" {
                        if let Some(diff) = result.result.get("diff").and_then(|v| v.as_str()) {
                            emit_diff_updated(
                                &outbound,
                                &store,
                                &chat_id,
                                &thread_id,
                                &turn_id_clone,
                                diff,
                            );
                        }
                    }
                }
                RunEventPayload::PlanUpdated { plan } => {
                    if super::debug_enabled() {
                        tracing::debug!(
                            %chat_id,
                            %thread_id,
                            %turn_id_clone,
                            plan_len = plan.len(),
                            "roci plan updated"
                        );
                    }
                    emit_plan_updated(
                        &outbound,
                        &store,
                        &chat_id,
                        &thread_id,
                        &turn_id_clone,
                        &plan,
                    );
                }
                RunEventPayload::DiffUpdated { diff } => {
                    if super::debug_enabled() {
                        tracing::debug!(
                            %chat_id,
                            %thread_id,
                            %turn_id_clone,
                            diff_len = diff.len(),
                            "roci diff updated"
                        );
                    }
                    emit_diff_updated(
                        &outbound,
                        &store,
                        &chat_id,
                        &thread_id,
                        &turn_id_clone,
                        &diff,
                    );
                }
                RunEventPayload::ApprovalRequired { request } => {
                    if super::debug_enabled() {
                        tracing::debug!(
                            %chat_id,
                            %thread_id,
                            %turn_id_clone,
                            request_id = %request.id,
                            kind = ?request.kind,
                            "roci approval required"
                        );
                    }
                    emit_approval_required(
                        &outbound,
                        &store,
                        &chat_id,
                        &thread_id,
                        &turn_id_clone,
                        &request,
                    );
                }
                RunEventPayload::Lifecycle { state: lifecycle } => {
                    if super::debug_enabled() {
                        tracing::debug!(
                            %chat_id,
                            %thread_id,
                            %turn_id_clone,
                            lifecycle = ?lifecycle,
                            "roci lifecycle event"
                        );
                    }
                    match lifecycle {
                        RunLifecycle::Completed => {
                            let snapshot = {
                                let mut guard = state.lock().await;
                                if let Some(thread) = guard.threads.get_mut(&thread_id) {
                                    thread.update_assistant_text(
                                        &assistant_item_id_clone,
                                        &assistant_text,
                                    );
                                    thread
                                        .messages
                                        .push(ModelMessage::assistant(assistant_text.clone()));
                                    thread.thread.updated_at = super::now_unix();
                                }
                                let snapshot = guard
                                    .threads
                                    .get(&thread_id)
                                    .map(PersistedThreadSnapshot::from_thread_state);
                                guard.runs.remove(&turn_id_clone);
                                if guard.active_threads.get(&thread_id) == Some(&turn_id_clone) {
                                    guard.active_threads.remove(&thread_id);
                                }
                                snapshot
                            };
                            persist_thread_snapshot(&store, &thread_id, snapshot);
                            if raw_events_enabled {
                                persist_roci_raw_event(
                                    &store,
                                    &turn_id_clone,
                                    &thread_id,
                                    "item/completed",
                                    serde_json::json!({
                                        "threadId": thread_id,
                                        "turnId": turn_id_clone,
                                        "item": {
                                            "id": assistant_item_id_clone,
                                            "type": "agentMessage",
                                            "text": assistant_text,
                                        },
                                    }),
                                );
                                let _ = store.prune_chat_raw_events(10);
                            }

                            emit_item_completed(
                                &outbound,
                                &store,
                                &chat_id,
                                &thread_id,
                                &turn_id_clone,
                                &assistant_item_id_clone,
                                &assistant_text,
                            );
                            if collaboration_mode.as_deref() == Some("plan")
                                && !assistant_text.trim().is_empty()
                            {
                                emit_plan_updated(
                                    &outbound,
                                    &store,
                                    &chat_id,
                                    &thread_id,
                                    &turn_id_clone,
                                    &assistant_text,
                                );
                            }
                            emit_turn_completed(
                                &outbound,
                                &store,
                                &chat_id,
                                &thread_id,
                                &turn_id_clone,
                                "completed",
                            );
                            if let Some(next) =
                                dequeue_next_run(&backend_for_task, &thread_id).await
                            {
                                spawn_next_run(
                                    backend_for_task.clone(),
                                    next,
                                    chat_id.clone(),
                                    thread_id.clone(),
                                );
                            }
                            break;
                        }
                        RunLifecycle::Failed { error } => {
                            let failure_text = if assistant_text.trim().is_empty() {
                                format!("Run failed: {error}")
                            } else {
                                assistant_text.clone()
                            };
                            emit_error(
                                &outbound,
                                &store,
                                &chat_id,
                                &thread_id,
                                &turn_id_clone,
                                error.clone(),
                            );
                            let snapshot = {
                                let mut guard = state.lock().await;
                                if let Some(thread) = guard.threads.get_mut(&thread_id) {
                                    thread.update_assistant_text(
                                        &assistant_item_id_clone,
                                        &failure_text,
                                    );
                                    thread
                                        .messages
                                        .push(ModelMessage::assistant(failure_text.clone()));
                                    thread.thread.updated_at = super::now_unix();
                                }
                                let snapshot = guard
                                    .threads
                                    .get(&thread_id)
                                    .map(PersistedThreadSnapshot::from_thread_state);
                                guard.runs.remove(&turn_id_clone);
                                if guard.active_threads.get(&thread_id) == Some(&turn_id_clone) {
                                    guard.active_threads.remove(&thread_id);
                                }
                                snapshot
                            };
                            persist_thread_snapshot(&store, &thread_id, snapshot);
                            if raw_events_enabled {
                                persist_roci_raw_event(
                                    &store,
                                    &turn_id_clone,
                                    &thread_id,
                                    "item/completed",
                                    serde_json::json!({
                                        "threadId": thread_id,
                                        "turnId": turn_id_clone,
                                        "item": {
                                            "id": assistant_item_id_clone,
                                            "type": "agentMessage",
                                            "text": failure_text,
                                        },
                                    }),
                                );
                                let _ = store.prune_chat_raw_events(10);
                            }
                            emit_item_completed(
                                &outbound,
                                &store,
                                &chat_id,
                                &thread_id,
                                &turn_id_clone,
                                &assistant_item_id_clone,
                                &failure_text,
                            );
                            emit_turn_completed(
                                &outbound,
                                &store,
                                &chat_id,
                                &thread_id,
                                &turn_id_clone,
                                "failed",
                            );
                            if let Some(next) =
                                dequeue_next_run(&backend_for_task, &thread_id).await
                            {
                                spawn_next_run(
                                    backend_for_task.clone(),
                                    next,
                                    chat_id.clone(),
                                    thread_id.clone(),
                                );
                            }
                            break;
                        }
                        RunLifecycle::Canceled => {
                            emit_turn_completed(
                                &outbound,
                                &store,
                                &chat_id,
                                &thread_id,
                                &turn_id_clone,
                                "canceled",
                            );
                            let snapshot = {
                                let mut guard = state.lock().await;
                                let snapshot = guard
                                    .threads
                                    .get(&thread_id)
                                    .map(PersistedThreadSnapshot::from_thread_state);
                                guard.runs.remove(&turn_id_clone);
                                if guard.active_threads.get(&thread_id) == Some(&turn_id_clone) {
                                    guard.active_threads.remove(&thread_id);
                                }
                                snapshot
                            };
                            persist_thread_snapshot(&store, &thread_id, snapshot);
                            if let Some(next) =
                                dequeue_next_run(&backend_for_task, &thread_id).await
                            {
                                spawn_next_run(
                                    backend_for_task.clone(),
                                    next,
                                    chat_id.clone(),
                                    thread_id.clone(),
                                );
                            }
                            break;
                        }
                        RunLifecycle::Started => {}
                    }
                }
                RunEventPayload::ToolCallDelta { .. } => {}
                RunEventPayload::ToolCallCompleted { .. } => {}
                RunEventPayload::Error { message } => {
                    tracing::debug!(%chat_id, %thread_id, %turn_id_clone, "run error event: {message}");
                }
            }
        }
    });

    Ok(())
}

pub(super) async fn dequeue_next_run(
    backend: &super::RociBackend,
    thread_id: &str,
) -> Option<PendingRun> {
    let mut state = backend.state.lock().await;
    let next = state
        .run_queue
        .get_mut(thread_id)
        .and_then(|queue| queue.pop_front());
    if let Some(run) = next.as_ref() {
        state
            .active_threads
            .insert(thread_id.to_string(), run.turn_id.clone());
    }
    next
}

fn spawn_next_run(
    backend: super::RociBackend,
    next: PendingRun,
    chat_id: String,
    thread_id: String,
) {
    tokio::task::spawn_blocking(move || {
        let handle = tokio::runtime::Handle::current();
        if let Err(err) = handle.block_on(start_run_inner(backend, next)) {
            if super::debug_enabled() {
                tracing::debug!(
                    %chat_id,
                    %thread_id,
                    error = %err,
                    "roci queued run start failed"
                );
            }
        }
    });
}

fn compact_messages(messages: &[ModelMessage]) -> Option<Vec<ModelMessage>> {
    if messages.len() <= 80 {
        return None;
    }
    Some(messages[messages.len().saturating_sub(80)..].to_vec())
}

fn trim_tool_result(mut result: roci::types::AgentToolResult) -> roci::types::AgentToolResult {
    if let Some(text) = result.result.as_str() {
        let truncated: String = text.chars().take(8000).collect();
        result.result = serde_json::Value::String(truncated);
        return result;
    }
    if let Some(obj) = result.result.as_object_mut() {
        if let Some(val) = obj.get_mut("output") {
            if let Some(text) = val.as_str() {
                let truncated: String = text.chars().take(8000).collect();
                *val = serde_json::Value::String(truncated);
            }
        }
    }
    result
}

fn exec_process_id_from_result(tool_name: &str, result: &serde_json::Value) -> Option<String> {
    if tool_name != "exec" {
        return None;
    }
    let status = result.get("status").and_then(|v| v.as_str());
    if status != Some("completed") {
        return None;
    }
    let truncated = result
        .get("stdout_truncated")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
        || result
            .get("stderr_truncated")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
    if !truncated {
        return None;
    }
    result
        .get("process_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}
