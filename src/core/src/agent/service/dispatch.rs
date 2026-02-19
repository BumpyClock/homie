use std::pin::Pin;
use std::sync::Arc;

use homie_protocol::{error_codes, Response};
use serde_json::Value;
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

use crate::outbound::OutboundMessage;
use crate::router::{ReapEvent, ServiceHandler};
use crate::storage::Store;
use crate::{ExecPolicy, HomieConfig};

use super::core::CodexChatCore;

pub struct ChatService {
    core: Arc<Mutex<CodexChatCore>>,
}

pub struct AgentService {
    core: Arc<Mutex<CodexChatCore>>,
}

impl ChatService {
    #[allow(dead_code)]
    pub fn new(
        outbound_tx: mpsc::Sender<OutboundMessage>,
        store: Arc<dyn Store>,
        homie_config: Arc<HomieConfig>,
        exec_policy: Arc<ExecPolicy>,
    ) -> Self {
        Self::new_with_channel(outbound_tx, store, homie_config, exec_policy, None)
    }

    pub fn new_with_channel(
        outbound_tx: mpsc::Sender<OutboundMessage>,
        store: Arc<dyn Store>,
        homie_config: Arc<HomieConfig>,
        exec_policy: Arc<ExecPolicy>,
        tool_channel: Option<String>,
    ) -> Self {
        Self {
            core: Arc::new(Mutex::new(CodexChatCore::new(
                outbound_tx,
                store,
                homie_config,
                exec_policy,
                tool_channel,
            ))),
        }
    }

    pub fn new_shared(
        outbound_tx: mpsc::Sender<OutboundMessage>,
        store: Arc<dyn Store>,
        homie_config: Arc<HomieConfig>,
        exec_policy: Arc<ExecPolicy>,
    ) -> (Self, AgentService) {
        Self::new_shared_with_channel(outbound_tx, store, homie_config, exec_policy, None)
    }

    pub fn new_shared_with_channel(
        outbound_tx: mpsc::Sender<OutboundMessage>,
        store: Arc<dyn Store>,
        homie_config: Arc<HomieConfig>,
        exec_policy: Arc<ExecPolicy>,
        tool_channel: Option<String>,
    ) -> (Self, AgentService) {
        let core = Arc::new(Mutex::new(CodexChatCore::new(
            outbound_tx,
            store,
            homie_config,
            exec_policy,
            tool_channel,
        )));
        (Self { core: core.clone() }, AgentService { core })
    }

    fn shutdown_core(&mut self) {
        let core = self.core.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let mut core = core.lock().await;
                core.shutdown();
            });
        } else if let Ok(mut core) = self.core.try_lock() {
            core.shutdown();
        }
    }

    fn reap_core(&mut self) -> Vec<ReapEvent> {
        self.core
            .try_lock()
            .map(|mut core| core.reap())
            .unwrap_or_default()
    }
}

impl AgentService {
    #[allow(dead_code)]
    pub fn new(
        outbound_tx: mpsc::Sender<OutboundMessage>,
        store: Arc<dyn Store>,
        homie_config: Arc<HomieConfig>,
        exec_policy: Arc<ExecPolicy>,
    ) -> Self {
        Self::new_with_channel(outbound_tx, store, homie_config, exec_policy, None)
    }

    pub fn new_with_channel(
        outbound_tx: mpsc::Sender<OutboundMessage>,
        store: Arc<dyn Store>,
        homie_config: Arc<HomieConfig>,
        exec_policy: Arc<ExecPolicy>,
        tool_channel: Option<String>,
    ) -> Self {
        Self {
            core: Arc::new(Mutex::new(CodexChatCore::new(
                outbound_tx,
                store,
                homie_config,
                exec_policy,
                tool_channel,
            ))),
        }
    }

    fn shutdown_core(&mut self) {
        let core = self.core.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let mut core = core.lock().await;
                core.shutdown();
            });
        } else if let Ok(mut core) = self.core.try_lock() {
            core.shutdown();
        }
    }

    fn reap_core(&mut self) -> Vec<ReapEvent> {
        self.core
            .try_lock()
            .map(|mut core| core.reap())
            .unwrap_or_default()
    }
}

impl ServiceHandler for ChatService {
    fn namespace(&self) -> &str {
        "chat"
    }

    fn handle_request(
        &mut self,
        id: Uuid,
        method: &str,
        params: Option<Value>,
    ) -> Pin<Box<dyn std::future::Future<Output = Response> + Send + '_>> {
        let method = method.to_string();
        Box::pin(async move {
            let mut core = self.core.lock().await;
            match method.as_str() {
                "chat.create" => core.chat_create(id).await,
                "chat.resume" => core.chat_resume(id, params).await,
                "chat.message.send" => core.chat_message_send(id, params).await,
                "chat.cancel" => core.chat_cancel(id, params).await,
                "chat.approval.respond" => core.approval_respond(id, params).await,
                "chat.list" => core.chat_list(id),
                "chat.thread.read" => core.chat_thread_read(id, params).await,
                "chat.thread.list" => core.chat_thread_list(id, params).await,
                "chat.thread.archive" => core.chat_thread_archive(id, params).await,
                "chat.thread.rename" => core.chat_thread_rename(id, params).await,
                "chat.settings.update" => core.chat_settings_update(id, params),
                "chat.files.search" => core.chat_files_search(id, params),
                "chat.account.read" => core.chat_account_read(id).await,
                "chat.account.list" => core.chat_account_list(id).await,
                "chat.account.login.start" => core.chat_account_login_start(id, params).await,
                "chat.account.login.poll" => core.chat_account_login_poll(id, params).await,
                "chat.skills.list" => core.chat_skills_list(id, params).await,
                "chat.model.list" => core.chat_model_list(id, params).await,
                "chat.tools.list" => core.chat_tools_list(id, params).await,
                "chat.collaboration.mode.list" => {
                    core.chat_collaboration_mode_list(id, params).await
                }
                "chat.skills.config.write" => core.chat_skills_config_write(id, params).await,
                _ => Response::error(
                    id,
                    error_codes::METHOD_NOT_FOUND,
                    format!("unknown method: {method}"),
                ),
            }
        })
    }

    fn handle_binary(&mut self, _frame: &homie_protocol::BinaryFrame) {
        tracing::debug!("chat service does not handle binary frames");
    }

    fn reap(&mut self) -> Vec<ReapEvent> {
        self.reap_core()
    }

    fn shutdown(&mut self) {
        self.shutdown_core();
    }
}

impl ServiceHandler for AgentService {
    fn namespace(&self) -> &str {
        "agent"
    }

    fn handle_request(
        &mut self,
        id: Uuid,
        method: &str,
        params: Option<Value>,
    ) -> Pin<Box<dyn std::future::Future<Output = Response> + Send + '_>> {
        let method = method.to_string();
        let canonical = match method.as_str() {
            "agent.codex.create" => "agent.chat.create",
            "agent.codex.message.send" => "agent.chat.message.send",
            "agent.codex.cancel" => "agent.chat.cancel",
            "agent.codex.approval.respond" => "agent.chat.approval.respond",
            "agent.codex.list" => "agent.chat.list",
            other => other,
        };
        let canonical = canonical.to_string();
        Box::pin(async move {
            let mut core = self.core.lock().await;
            match canonical.as_str() {
                "agent.chat.create" => core.chat_create(id).await,
                "agent.chat.message.send" => core.chat_message_send(id, params).await,
                "agent.chat.cancel" => core.chat_cancel(id, params).await,
                "agent.chat.approval.respond" => core.approval_respond(id, params).await,
                "agent.chat.list" => core.chat_list(id),
                _ => Response::error(
                    id,
                    error_codes::METHOD_NOT_FOUND,
                    format!("unknown method: {method}"),
                ),
            }
        })
    }

    fn handle_binary(&mut self, _frame: &homie_protocol::BinaryFrame) {
        tracing::debug!("agent service does not handle binary frames");
    }

    fn reap(&mut self) -> Vec<ReapEvent> {
        self.reap_core()
    }

    fn shutdown(&mut self) {
        self.shutdown_core();
    }
}

impl Drop for ChatService {
    fn drop(&mut self) {
        if let Ok(mut core) = self.core.try_lock() {
            core.shutdown();
        }
    }
}

impl Drop for AgentService {
    fn drop(&mut self) {
        if let Ok(mut core) = self.core.try_lock() {
            core.shutdown();
        }
    }
}
