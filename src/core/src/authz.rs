use crate::auth::AuthOutcome;
use crate::config::ServerConfig;

/// Roles used for per-method authorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Owner,
    User,
    Viewer,
}

/// Scopes required by specific methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    TerminalRead,
    TerminalWrite,
    AgentRead,
    AgentWrite,
    Events,
    PresenceRead,
    PresenceWrite,
    JobsRead,
    JobsWrite,
    PairingRead,
    PairingWrite,
    NotificationsRead,
    NotificationsWrite,
}

/// Authorization context derived from the authenticated connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthContext {
    role: Role,
}

impl AuthContext {
    pub fn new(role: Role) -> Self {
        Self { role }
    }

    pub fn role(&self) -> Role {
        self.role
    }

    pub fn allows(&self, scope: Scope) -> bool {
        match self.role {
            Role::Owner => true,
            Role::User => matches!(
                scope,
                Scope::TerminalRead
                    | Scope::TerminalWrite
                    | Scope::AgentRead
                    | Scope::AgentWrite
                    | Scope::Events
                    | Scope::PresenceRead
                    | Scope::PresenceWrite
            ),
            Role::Viewer => matches!(
                scope,
                Scope::TerminalRead | Scope::AgentRead | Scope::Events | Scope::PresenceRead
            ),
        }
    }
}

pub fn context_for_outcome(outcome: &AuthOutcome, config: &ServerConfig) -> AuthContext {
    let role = match outcome {
        AuthOutcome::Local => config.local_role,
        AuthOutcome::Lan => config.local_role,
        AuthOutcome::Tailscale(_) => config.tailscale_role,
        AuthOutcome::Rejected(_) => Role::Viewer,
    };
    AuthContext::new(role)
}

pub fn scope_for_method(method: &str) -> Option<Scope> {
    match method {
        "events.subscribe" | "events.unsubscribe" => Some(Scope::Events),
        "terminal.session.list"
        | "terminal.session.attach"
        | "terminal.session.preview"
        | "terminal.tmux.list" => Some(Scope::TerminalRead),
        "terminal.session.start"
        | "terminal.session.resize"
        | "terminal.session.input"
        | "terminal.session.kill"
        | "terminal.session.remove"
        | "terminal.session.rename"
        | "terminal.tmux.attach"
        | "terminal.tmux.kill" => Some(Scope::TerminalWrite),
        "agent.chat.list" | "agent.codex.list" => Some(Scope::AgentRead),
        "chat.list"
        | "chat.thread.read"
        | "chat.thread.list"
        | "chat.account.read"
        | "chat.account.list"
        | "chat.skills.list"
        | "chat.model.list"
        | "chat.collaboration.mode.list"
        | "chat.files.search" => Some(Scope::AgentRead),
        "agent.chat.create"
        | "agent.chat.message.send"
        | "agent.chat.cancel"
        | "agent.chat.approval.respond"
        | "agent.codex.create"
        | "agent.codex.message.send"
        | "agent.codex.cancel"
        | "agent.codex.approval.respond"
        | "chat.create"
        | "chat.resume"
        | "chat.message.send"
        | "chat.cancel"
        | "chat.approval.respond"
        | "chat.thread.archive"
        | "chat.thread.rename"
        | "chat.settings.update"
        | "chat.skills.config.write"
        | "chat.account.login.start"
        | "chat.account.login.poll" => Some(Scope::AgentWrite),
        "presence.list" => Some(Scope::PresenceRead),
        "presence.register" | "presence.heartbeat" | "presence.unregister" => {
            Some(Scope::PresenceWrite)
        }
        "jobs.status" | "jobs.logs.tail" => Some(Scope::JobsRead),
        "jobs.start" | "jobs.cancel" => Some(Scope::JobsWrite),
        "pairing.list" => Some(Scope::PairingRead),
        "pairing.request" | "pairing.approve" | "pairing.revoke" => Some(Scope::PairingWrite),
        "notifications.list" => Some(Scope::NotificationsRead),
        "notifications.register" | "notifications.send" => Some(Scope::NotificationsWrite),
        "agent.chat.event.subscribe"
        | "agent.codex.event.subscribe"
        | "chat.event.subscribe" => Some(Scope::Events),
        _ => None,
    }
}
