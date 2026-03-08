use symphony_config::RuntimeConfig;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostOwnedConfigChange {
    TrackerKind,
    TrackerEndpoint,
    TrackerApiKey,
    TrackerProjectSlug,
    TrackerActiveStates,
    WorkspaceRoot,
    ServerHost,
    ServerPort,
    LogLevel,
    ObservabilityEnabled,
}

impl HostOwnedConfigChange {
    #[must_use]
    pub const fn field_path(self) -> &'static str {
        match self {
            Self::TrackerKind => "tracker.kind",
            Self::TrackerEndpoint => "tracker.endpoint",
            Self::TrackerApiKey => "tracker.api_key",
            Self::TrackerProjectSlug => "tracker.project_slug",
            Self::TrackerActiveStates => "tracker.active_states",
            Self::WorkspaceRoot => "workspace.root",
            Self::ServerHost => "server.host",
            Self::ServerPort => "server.port",
            Self::LogLevel => "log_level.level",
            Self::ObservabilityEnabled => "observability.dashboard_enabled",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkflowReloadDisposition {
    LiveApply,
    RestartRequired { reasons: Vec<HostOwnedConfigChange> },
}

impl WorkflowReloadDisposition {
    #[must_use]
    pub const fn requires_restart(&self) -> bool {
        matches!(self, Self::RestartRequired { .. })
    }

    #[must_use]
    pub fn restart_reasons(&self) -> &[HostOwnedConfigChange] {
        match self {
            Self::LiveApply => &[],
            Self::RestartRequired { reasons } => reasons,
        }
    }
}

#[must_use]
pub fn classify_workflow_reload(
    current: &RuntimeConfig,
    next: &RuntimeConfig,
) -> WorkflowReloadDisposition {
    let mut reasons = Vec::new();

    if current.tracker.kind != next.tracker.kind {
        reasons.push(HostOwnedConfigChange::TrackerKind);
    }
    if current.tracker.endpoint != next.tracker.endpoint {
        reasons.push(HostOwnedConfigChange::TrackerEndpoint);
    }
    if current.tracker.api_key != next.tracker.api_key {
        reasons.push(HostOwnedConfigChange::TrackerApiKey);
    }
    if current.tracker.project_slug != next.tracker.project_slug {
        reasons.push(HostOwnedConfigChange::TrackerProjectSlug);
    }
    if current.tracker.active_states != next.tracker.active_states {
        reasons.push(HostOwnedConfigChange::TrackerActiveStates);
    }
    if current.workspace.root != next.workspace.root {
        reasons.push(HostOwnedConfigChange::WorkspaceRoot);
    }
    if current.server.host != next.server.host {
        reasons.push(HostOwnedConfigChange::ServerHost);
    }
    if current.server.port != next.server.port {
        reasons.push(HostOwnedConfigChange::ServerPort);
    }
    if current.log_level.level != next.log_level.level {
        reasons.push(HostOwnedConfigChange::LogLevel);
    }
    if current.observability.enabled != next.observability.enabled {
        reasons.push(HostOwnedConfigChange::ObservabilityEnabled);
    }

    if reasons.is_empty() {
        WorkflowReloadDisposition::LiveApply
    } else {
        WorkflowReloadDisposition::RestartRequired { reasons }
    }
}

#[cfg(test)]
mod tests {
    use symphony_config::RuntimeConfig;

    use super::{HostOwnedConfigChange, WorkflowReloadDisposition, classify_workflow_reload};

    #[test]
    fn classifies_runtime_safe_changes_as_live_applicable() {
        let current = RuntimeConfig::default();
        let mut next = current.clone();
        next.polling.interval_ms = 45_000;
        next.agent.max_turns = 42;
        next.observability.refresh_ms = 2_000;
        next.observability.render_interval_ms = 33;
        next.version = 99;

        assert_eq!(
            classify_workflow_reload(&current, &next),
            WorkflowReloadDisposition::LiveApply
        );
    }

    #[test]
    fn classifies_server_port_change_as_restart_required() {
        let current = RuntimeConfig::default();
        let mut next = current.clone();
        next.server.port = Some(4051);

        assert_eq!(
            classify_workflow_reload(&current, &next),
            WorkflowReloadDisposition::RestartRequired {
                reasons: vec![HostOwnedConfigChange::ServerPort],
            }
        );
    }

    #[test]
    fn classifies_server_host_change_as_restart_required() {
        let current = RuntimeConfig::default();
        let mut next = current.clone();
        next.server.host = std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED);

        assert_eq!(
            classify_workflow_reload(&current, &next),
            WorkflowReloadDisposition::RestartRequired {
                reasons: vec![HostOwnedConfigChange::ServerHost],
            }
        );
    }

    #[test]
    fn classifies_tracker_changes_as_restart_required() {
        let current = RuntimeConfig::default();
        let mut next = current.clone();
        next.tracker.kind = "mock-tracker".to_owned();
        next.tracker.endpoint = "https://example.invalid/graphql".to_owned();
        next.tracker.api_key = Some("new-token".to_owned());
        next.tracker.project_slug = Some("NEW".to_owned());
        next.tracker.active_states = vec!["Todo".to_owned(), "Doing".to_owned()];

        assert_eq!(
            classify_workflow_reload(&current, &next),
            WorkflowReloadDisposition::RestartRequired {
                reasons: vec![
                    HostOwnedConfigChange::TrackerKind,
                    HostOwnedConfigChange::TrackerEndpoint,
                    HostOwnedConfigChange::TrackerApiKey,
                    HostOwnedConfigChange::TrackerProjectSlug,
                    HostOwnedConfigChange::TrackerActiveStates,
                ],
            }
        );
    }

    #[test]
    fn classifies_log_level_change_as_restart_required() {
        let current = RuntimeConfig::default();
        let mut next = current.clone();
        next.log_level.level = "debug".to_owned();

        let disposition = classify_workflow_reload(&current, &next);
        assert!(disposition.requires_restart());
        assert_eq!(
            disposition.restart_reasons(),
            &[HostOwnedConfigChange::LogLevel]
        );
    }

    #[test]
    fn classifies_observability_enablement_change_as_restart_required() {
        let current = RuntimeConfig::default();
        let mut next = current.clone();
        next.observability.enabled = !current.observability.enabled;

        let disposition = classify_workflow_reload(&current, &next);
        assert!(disposition.requires_restart());
        assert_eq!(
            disposition.restart_reasons(),
            &[HostOwnedConfigChange::ObservabilityEnabled]
        );
    }

    #[test]
    fn classifies_workspace_root_change_as_restart_required() {
        let current = RuntimeConfig::default();
        let mut next = current.clone();
        next.workspace.root = std::env::temp_dir().join("symphony-new-root");

        assert_eq!(
            classify_workflow_reload(&current, &next),
            WorkflowReloadDisposition::RestartRequired {
                reasons: vec![HostOwnedConfigChange::WorkspaceRoot],
            }
        );
    }

    #[test]
    fn exposes_restart_reason_field_paths() {
        assert_eq!(
            [
                HostOwnedConfigChange::TrackerKind,
                HostOwnedConfigChange::TrackerEndpoint,
                HostOwnedConfigChange::TrackerApiKey,
                HostOwnedConfigChange::TrackerProjectSlug,
                HostOwnedConfigChange::TrackerActiveStates,
                HostOwnedConfigChange::WorkspaceRoot,
                HostOwnedConfigChange::ServerHost,
                HostOwnedConfigChange::ServerPort,
                HostOwnedConfigChange::LogLevel,
                HostOwnedConfigChange::ObservabilityEnabled,
            ]
            .map(HostOwnedConfigChange::field_path),
            [
                "tracker.kind",
                "tracker.endpoint",
                "tracker.api_key",
                "tracker.project_slug",
                "tracker.active_states",
                "workspace.root",
                "server.host",
                "server.port",
                "log_level.level",
                "observability.dashboard_enabled",
            ]
        );
    }
}
