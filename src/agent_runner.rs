use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};

use crate::agent::{
    Agent, AgentContext, AgentPaths, AgentSettings, ClaudeAgent, CodexAgent, CommandSpec,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum AgentKind {
    Claude,
    Codex,
}

#[derive(Clone, Debug)]
pub struct AgentRunnerOptions {
    pub kind: AgentKind,
    pub name: String,
    pub model: Option<String>,
    pub permission_mode: String,
    pub tmux_session: Option<String>,
    pub agent_dir: Option<PathBuf>,
    pub system_prompt: Option<PathBuf>,
    pub sandbox: bool,
}

#[derive(Clone, Debug)]
pub struct AgentStartOptions {
    pub kind: AgentKind,
    pub name: String,
    pub binary: Option<PathBuf>,
    pub agent_dir: PathBuf,
    pub model: String,
    pub permission_mode: String,
    pub initial_prompt: String,
    pub system_prompt: String,
    pub auto_continue_prompt: String,
    pub tmux_session: Option<String>,
    pub env: Vec<(String, String)>,
    pub extra_args: Vec<String>,
    pub sandbox: bool,
    pub bare: bool,
}

#[derive(Clone, Debug)]
pub struct AgentSendOptions {
    pub kind: AgentKind,
    pub name: String,
    pub binary: Option<PathBuf>,
    pub agent_dir: PathBuf,
    pub model: String,
    pub permission_mode: String,
    pub prompt: String,
    pub auto_continue_prompt: String,
    pub tmux_session: Option<String>,
    pub env: Vec<(String, String)>,
    pub extra_args: Vec<String>,
    pub sandbox: bool,
    pub bare: bool,
}

#[derive(Clone, Debug)]
pub struct AgentStopOptions {
    pub kind: AgentKind,
    pub name: String,
    pub binary: Option<PathBuf>,
    pub agent_dir: PathBuf,
    pub model: String,
    pub permission_mode: String,
    pub auto_continue_prompt: String,
    pub tmux_session: Option<String>,
    pub env: Vec<(String, String)>,
    pub extra_args: Vec<String>,
    pub sandbox: bool,
    pub bare: bool,
    pub grace_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartedAgent {
    pub name: String,
    pub kind: &'static str,
    pub tmux_session: String,
    pub agent_dir: PathBuf,
    pub command_file: PathBuf,
    pub events_file: PathBuf,
    pub attach_command: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoppedAgent {
    pub name: String,
    pub kind: &'static str,
    pub tmux_session: Option<String>,
    pub agent_dir: PathBuf,
    pub events_file: PathBuf,
    pub log_file: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ActiveAgent {
    name: String,
    kind: String,
    operation: String,
    tmux_session: String,
    command_file: PathBuf,
    started_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OperationReport {
    pub time: String,
    pub operation: String,
    pub status: String,
    pub agent_name: String,
    pub agent_kind: String,
    pub agent_dir: PathBuf,
    pub tmux_session: Option<String>,
    pub attach_command: Option<String>,
    pub command_file: Option<PathBuf>,
    pub log_file: Option<PathBuf>,
    pub events_file: PathBuf,
    pub error: Option<String>,
}

pub fn start_agent_tmux_for_local_world(
    world_dir: &Path,
    port: u16,
    base_path: &str,
    options: AgentRunnerOptions,
    bundled_prompt: Option<String>,
) -> Result<StartedAgent, String> {
    let safe_name = sanitize_agent_name(&options.name);
    let agent_dir = options
        .agent_dir
        .clone()
        .unwrap_or_else(|| world_dir.join(".clawblox").join("agents").join(&safe_name));
    start_agent(AgentStartOptions {
        kind: options.kind,
        name: options.name,
        binary: None,
        agent_dir,
        model: options
            .model
            .unwrap_or_else(|| default_model(options.kind).to_string()),
        permission_mode: options.permission_mode,
        initial_prompt: "Begin".to_string(),
        system_prompt: match options.system_prompt.as_ref() {
            Some(path) => std::fs::read_to_string(path)
                .map_err(|err| format!("failed to read system prompt {}: {err}", path.display()))?,
            None => {
                bundled_prompt.unwrap_or_else(|| "Use the local Clawblox world API.".to_string())
            }
        },
        auto_continue_prompt: "Please don't stop. Just continue doing whatever you want to do."
            .to_string(),
        tmux_session: options.tmux_session,
        env: vec![
            (
                "WORLD_BASE_URL".to_string(),
                local_world_url(port, base_path),
            ),
            (
                "WORLD_INTERNAL_BASE_URL".to_string(),
                local_world_url(port, base_path),
            ),
            ("WORLD_AGENT_NAME".to_string(), safe_name),
        ],
        extra_args: Vec::new(),
        sandbox: options.sandbox,
        bare: false,
    })
}

pub fn start_agent(options: AgentStartOptions) -> Result<StartedAgent, String> {
    let safe_name = sanitize_agent_name(&options.name);
    let agent_dir = absolutize_path(&options.agent_dir);
    let kind_name = match options.kind {
        AgentKind::Claude => "claude",
        AgentKind::Codex => "codex",
    };
    let runtime_dir = agent_dir.join("runtime");
    let log_dir = agent_dir.join("logs");
    let events_file = agent_dir.join("events.jsonl");
    let ctx = AgentContext {
        paths: AgentPaths {
            workspace_dir: agent_dir.join("workspace"),
            prompt_file: runtime_dir.join("system_prompt.md"),
            session_id_file: agent_dir.join(format!("{kind_name}_session_id.txt")),
            transcript_dir: agent_dir.join("session"),
            runtime_dir,
            log_file: log_dir.join("agent.log"),
            agent_home_dir: agent_dir.join("home"),
        },
        settings: AgentSettings {
            model: options.model.clone(),
            permission_mode: options.permission_mode.clone(),
            initial_prompt: options.initial_prompt.clone(),
            lifecycle_prompt:
                "You will be reset in 5 minutes. Update your workspace memory files now."
                    .to_string(),
            auto_continue_prompt: options.auto_continue_prompt.clone(),
            extra_args: options.extra_args.clone(),
            env: options.env.clone(),
            sandbox_enabled: options.sandbox,
        },
    };

    if let Err(err) = ctx.prepare_paths() {
        let message = format!("failed to prepare agent paths: {err}");
        let _ = append_operation_report(&events_file, failure_report(&ctx, &options, &message));
        return Err(message);
    }
    if let Err(err) = std::fs::write(&ctx.paths.prompt_file, &options.system_prompt) {
        let message = format!(
            "failed to write agent prompt {}: {err}",
            ctx.paths.prompt_file.display()
        );
        let _ = append_operation_report(&events_file, failure_report(&ctx, &options, &message));
        return Err(message);
    }

    let agent = build_agent(options.kind, options.binary.clone(), options.bare);

    if let Err(err) = agent.check_setup(&ctx) {
        let message = format!("{} setup failed: {err}", agent.name());
        let _ = append_operation_report(&events_file, failure_report(&ctx, &options, &message));
        return Err(message);
    }
    if let Err(err) = agent.configure_auto_continue(&ctx) {
        let message = format!("failed to configure {} hooks: {err}", agent.name());
        let _ = append_operation_report(&events_file, failure_report(&ctx, &options, &message));
        return Err(message);
    }
    let command = match agent.start(&ctx) {
        Ok(command) => command,
        Err(err) => {
            let message = format!("failed to build {} command: {err}", agent.name());
            let _ = append_operation_report(&events_file, failure_report(&ctx, &options, &message));
            return Err(message);
        }
    };

    let tmux_session = options
        .tmux_session
        .clone()
        .unwrap_or_else(|| format!("clawblox-{}-{}", agent.name(), safe_name));
    if let Err(message) = ensure_no_active_agent(&ctx) {
        let _ = append_operation_report(&events_file, failure_report(&ctx, &options, &message));
        return Err(message);
    }
    if tmux_session_exists(&tmux_session) {
        let message = format!(
            "tmux session '{}' already exists. Use --tmux-session or close the old session.",
            tmux_session
        );
        let _ = append_operation_report(&events_file, failure_report(&ctx, &options, &message));
        return Err(message);
    }

    let command_file = ctx.paths.runtime_dir.join("start.sh");
    let shell_command = command_spec_to_shell(&command);
    if let Err(err) = write_command_file(&command_file, &shell_command) {
        let message = format!(
            "failed to write command file {}: {err}",
            command_file.display()
        );
        let _ = append_operation_report(&events_file, failure_report(&ctx, &options, &message));
        return Err(message);
    }

    let attach_command = format!("tmux attach -t {}", tmux_session);
    append_operation_report(
        &events_file,
        OperationReport {
            time: utc_now(),
            operation: "agent.start".to_string(),
            status: "starting".to_string(),
            agent_name: options.name.clone(),
            agent_kind: agent.name().to_string(),
            agent_dir: agent_dir.clone(),
            tmux_session: Some(tmux_session.clone()),
            attach_command: Some(attach_command.clone()),
            command_file: Some(command_file.clone()),
            log_file: Some(ctx.paths.log_file.clone()),
            events_file: events_file.clone(),
            error: None,
        },
    )
    .map_err(|err| {
        format!(
            "failed to write operation event {}: {err}",
            events_file.display()
        )
    })?;

    if let Err(err) = start_tmux_session(
        &tmux_session,
        &command_file,
        &ctx.paths.log_file,
        &command.env,
    ) {
        let message = format!(
            "{err}\nagent dir: {}\ncommand file: {}\nevents file: {}\nattach: {}",
            agent_dir.display(),
            command_file.display(),
            events_file.display(),
            attach_command
        );
        let _ = append_operation_report(&events_file, failure_report(&ctx, &options, &message));
        return Err(message);
    }
    if let Err(err) = write_active_agent(
        &ctx,
        ActiveAgent {
            name: options.name.clone(),
            kind: agent.name().to_string(),
            operation: "agent.start".to_string(),
            tmux_session: tmux_session.clone(),
            command_file: command_file.clone(),
            started_at: utc_now(),
        },
    ) {
        let message = format!(
            "failed to write active agent state {}: {err}",
            active_agent_file(&ctx).display()
        );
        let _ = append_operation_report(&events_file, failure_report(&ctx, &options, &message));
        return Err(message);
    }

    append_operation_report(
        &events_file,
        OperationReport {
            time: utc_now(),
            operation: "agent.start".to_string(),
            status: "started".to_string(),
            agent_name: options.name.clone(),
            agent_kind: agent.name().to_string(),
            agent_dir: agent_dir.clone(),
            tmux_session: Some(tmux_session.clone()),
            attach_command: Some(attach_command.clone()),
            command_file: Some(command_file.clone()),
            log_file: Some(ctx.paths.log_file.clone()),
            events_file: events_file.clone(),
            error: None,
        },
    )
    .map_err(|err| {
        format!(
            "failed to write operation event {}: {err}",
            events_file.display()
        )
    })?;

    Ok(StartedAgent {
        name: options.name,
        kind: agent.name(),
        tmux_session,
        agent_dir,
        command_file,
        events_file,
        attach_command,
    })
}

pub fn send_agent(options: AgentSendOptions) -> Result<StartedAgent, String> {
    let safe_name = sanitize_agent_name(&options.name);
    let agent_dir = absolutize_path(&options.agent_dir);
    let kind_name = match options.kind {
        AgentKind::Claude => "claude",
        AgentKind::Codex => "codex",
    };
    let runtime_dir = agent_dir.join("runtime");
    let log_dir = agent_dir.join("logs");
    let events_file = agent_dir.join("events.jsonl");
    let ctx = AgentContext {
        paths: AgentPaths {
            workspace_dir: agent_dir.join("workspace"),
            prompt_file: runtime_dir.join("system_prompt.md"),
            session_id_file: agent_dir.join(format!("{kind_name}_session_id.txt")),
            transcript_dir: agent_dir.join("session"),
            runtime_dir,
            log_file: log_dir.join("agent.log"),
            agent_home_dir: agent_dir.join("home"),
        },
        settings: AgentSettings {
            model: options.model.clone(),
            permission_mode: options.permission_mode.clone(),
            initial_prompt: "Begin".to_string(),
            lifecycle_prompt:
                "You will be reset in 5 minutes. Update your workspace memory files now."
                    .to_string(),
            auto_continue_prompt: options.auto_continue_prompt.clone(),
            extra_args: options.extra_args.clone(),
            env: options.env.clone(),
            sandbox_enabled: options.sandbox,
        },
    };

    if let Err(err) = ctx.prepare_paths() {
        let message = format!("failed to prepare agent paths: {err}");
        let _ =
            append_operation_report(&events_file, send_failure_report(&ctx, &options, &message));
        return Err(message);
    }

    let agent = build_agent(options.kind, options.binary.clone(), options.bare);

    if let Err(err) = agent.check_setup(&ctx) {
        let message = format!("{} setup failed: {err}", agent.name());
        let _ =
            append_operation_report(&events_file, send_failure_report(&ctx, &options, &message));
        return Err(message);
    }
    if let Err(err) = agent.configure_auto_continue(&ctx) {
        let message = format!("failed to configure {} hooks: {err}", agent.name());
        let _ =
            append_operation_report(&events_file, send_failure_report(&ctx, &options, &message));
        return Err(message);
    }
    let command = match agent.continue_session(&ctx, &options.prompt) {
        Ok(command) => command,
        Err(err) => {
            let message = format!("failed to build {} send command: {err}", agent.name());
            let _ = append_operation_report(
                &events_file,
                send_failure_report(&ctx, &options, &message),
            );
            return Err(message);
        }
    };

    let tmux_session = options
        .tmux_session
        .clone()
        .unwrap_or_else(|| format!("clawblox-{}-{}-send", agent.name(), safe_name));
    if let Err(message) = ensure_no_active_agent(&ctx) {
        let _ =
            append_operation_report(&events_file, send_failure_report(&ctx, &options, &message));
        return Err(message);
    }
    if tmux_session_exists(&tmux_session) {
        let message = format!(
            "tmux session '{}' already exists. Stop the current agent process before sending another prompt.",
            tmux_session
        );
        let _ =
            append_operation_report(&events_file, send_failure_report(&ctx, &options, &message));
        return Err(message);
    }

    let command_file = ctx.paths.runtime_dir.join("send.sh");
    let shell_command = command_spec_to_shell(&command);
    if let Err(err) = write_command_file(&command_file, &shell_command) {
        let message = format!(
            "failed to write command file {}: {err}",
            command_file.display()
        );
        let _ =
            append_operation_report(&events_file, send_failure_report(&ctx, &options, &message));
        return Err(message);
    }

    let attach_command = format!("tmux attach -t {}", tmux_session);
    append_operation_report(
        &events_file,
        OperationReport {
            time: utc_now(),
            operation: "agent.send".to_string(),
            status: "starting".to_string(),
            agent_name: options.name.clone(),
            agent_kind: agent.name().to_string(),
            agent_dir: agent_dir.clone(),
            tmux_session: Some(tmux_session.clone()),
            attach_command: Some(attach_command.clone()),
            command_file: Some(command_file.clone()),
            log_file: Some(ctx.paths.log_file.clone()),
            events_file: events_file.clone(),
            error: None,
        },
    )
    .map_err(|err| {
        format!(
            "failed to write operation event {}: {err}",
            events_file.display()
        )
    })?;

    if let Err(err) = start_tmux_session(
        &tmux_session,
        &command_file,
        &ctx.paths.log_file,
        &command.env,
    ) {
        let message = format!(
            "{err}\nagent dir: {}\ncommand file: {}\nevents file: {}\nattach: {}",
            agent_dir.display(),
            command_file.display(),
            events_file.display(),
            attach_command
        );
        let _ =
            append_operation_report(&events_file, send_failure_report(&ctx, &options, &message));
        return Err(message);
    }
    if let Err(err) = write_active_agent(
        &ctx,
        ActiveAgent {
            name: options.name.clone(),
            kind: agent.name().to_string(),
            operation: "agent.send".to_string(),
            tmux_session: tmux_session.clone(),
            command_file: command_file.clone(),
            started_at: utc_now(),
        },
    ) {
        let message = format!(
            "failed to write active agent state {}: {err}",
            active_agent_file(&ctx).display()
        );
        let _ =
            append_operation_report(&events_file, send_failure_report(&ctx, &options, &message));
        return Err(message);
    }

    append_operation_report(
        &events_file,
        OperationReport {
            time: utc_now(),
            operation: "agent.send".to_string(),
            status: "started".to_string(),
            agent_name: options.name.clone(),
            agent_kind: agent.name().to_string(),
            agent_dir: agent_dir.clone(),
            tmux_session: Some(tmux_session.clone()),
            attach_command: Some(attach_command.clone()),
            command_file: Some(command_file.clone()),
            log_file: Some(ctx.paths.log_file.clone()),
            events_file: events_file.clone(),
            error: None,
        },
    )
    .map_err(|err| {
        format!(
            "failed to write operation event {}: {err}",
            events_file.display()
        )
    })?;

    Ok(StartedAgent {
        name: options.name,
        kind: agent.name(),
        tmux_session,
        agent_dir,
        command_file,
        events_file,
        attach_command,
    })
}

pub fn stop_agent(options: AgentStopOptions) -> Result<StoppedAgent, String> {
    let agent_dir = absolutize_path(&options.agent_dir);
    let kind_name = match options.kind {
        AgentKind::Claude => "claude",
        AgentKind::Codex => "codex",
    };
    let runtime_dir = agent_dir.join("runtime");
    let log_dir = agent_dir.join("logs");
    let events_file = agent_dir.join("events.jsonl");
    let ctx = AgentContext {
        paths: AgentPaths {
            workspace_dir: agent_dir.join("workspace"),
            prompt_file: runtime_dir.join("system_prompt.md"),
            session_id_file: agent_dir.join(format!("{kind_name}_session_id.txt")),
            transcript_dir: agent_dir.join("session"),
            runtime_dir,
            log_file: log_dir.join("agent.log"),
            agent_home_dir: agent_dir.join("home"),
        },
        settings: AgentSettings {
            model: options.model.clone(),
            permission_mode: options.permission_mode.clone(),
            initial_prompt: "Begin".to_string(),
            lifecycle_prompt:
                "You will be reset in 5 minutes. Update your workspace memory files now."
                    .to_string(),
            auto_continue_prompt: options.auto_continue_prompt.clone(),
            extra_args: options.extra_args.clone(),
            env: options.env.clone(),
            sandbox_enabled: options.sandbox,
        },
    };

    if let Err(err) = ctx.prepare_paths() {
        let message = format!("failed to prepare agent paths: {err}");
        let _ =
            append_operation_report(&events_file, stop_failure_report(&ctx, &options, &message));
        return Err(message);
    }

    let agent = build_agent(options.kind, options.binary.clone(), options.bare);

    let active = match read_active_agent(&ctx) {
        Ok(active) => active,
        Err(err) => {
            let message = format!(
                "failed to read active agent state {}: {err}",
                active_agent_file(&ctx).display()
            );
            let _ = append_operation_report(
                &events_file,
                stop_failure_report(&ctx, &options, &message),
            );
            return Err(message);
        }
    };
    let tmux_session = active
        .as_ref()
        .map(|active| active.tmux_session.clone())
        .or_else(|| options.tmux_session.clone());

    append_operation_report(
        &events_file,
        OperationReport {
            time: utc_now(),
            operation: "agent.stop".to_string(),
            status: "stopping".to_string(),
            agent_name: options.name.clone(),
            agent_kind: agent.name().to_string(),
            agent_dir: agent_dir.clone(),
            tmux_session: tmux_session.clone(),
            attach_command: tmux_session
                .as_ref()
                .map(|session| format!("tmux attach -t {session}")),
            command_file: active.as_ref().map(|active| active.command_file.clone()),
            log_file: Some(ctx.paths.log_file.clone()),
            events_file: events_file.clone(),
            error: None,
        },
    )
    .map_err(|err| {
        format!(
            "failed to write operation event {}: {err}",
            events_file.display()
        )
    })?;

    if let Some(session) = &tmux_session {
        if tmux_session_exists(session) {
            send_tmux_ctrl_c(session);
            std::thread::sleep(std::time::Duration::from_secs(options.grace_seconds));
            if tmux_session_exists(session) {
                kill_tmux_session(session);
            }
            if tmux_session_exists(session) {
                let message = format!(
                    "tmux session '{}' still exists after stop. Attach with: tmux attach -t {}",
                    session, session
                );
                let _ = append_operation_report(
                    &events_file,
                    stop_failure_report(&ctx, &options, &message),
                );
                return Err(message);
            }
        }
    }

    if let Err(err) = agent.collect_transcripts(&ctx) {
        let message = format!("failed to collect {} transcript: {err}", agent.name());
        let _ =
            append_operation_report(&events_file, stop_failure_report(&ctx, &options, &message));
        return Err(message);
    }
    let _ = std::fs::remove_file(active_agent_file(&ctx));

    append_operation_report(
        &events_file,
        OperationReport {
            time: utc_now(),
            operation: "agent.stop".to_string(),
            status: "stopped".to_string(),
            agent_name: options.name.clone(),
            agent_kind: agent.name().to_string(),
            agent_dir: agent_dir.clone(),
            tmux_session: tmux_session.clone(),
            attach_command: None,
            command_file: active.as_ref().map(|active| active.command_file.clone()),
            log_file: Some(ctx.paths.log_file.clone()),
            events_file: events_file.clone(),
            error: None,
        },
    )
    .map_err(|err| {
        format!(
            "failed to write operation event {}: {err}",
            events_file.display()
        )
    })?;

    Ok(StoppedAgent {
        name: options.name,
        kind: agent.name(),
        tmux_session,
        agent_dir,
        events_file,
        log_file: ctx.paths.log_file,
    })
}

fn default_model(kind: AgentKind) -> &'static str {
    match kind {
        AgentKind::Claude => "claude-opus-4-6",
        AgentKind::Codex => "gpt-5.5",
    }
}

fn build_agent(kind: AgentKind, binary: Option<PathBuf>, bare: bool) -> Box<dyn Agent> {
    match kind {
        AgentKind::Claude => Box::new(ClaudeAgent {
            binary: binary.unwrap_or_else(|| ClaudeAgent::default().binary),
            bare,
            ..ClaudeAgent::default()
        }),
        AgentKind::Codex => Box::new(CodexAgent {
            binary: binary.unwrap_or_else(|| CodexAgent::default().binary),
        }),
    }
}

fn local_world_url(port: u16, base_path: &str) -> String {
    format!("http://localhost:{port}{}", base_path.trim_end_matches('/'))
}

fn tmux_session_exists(name: &str) -> bool {
    Command::new("tmux")
        .args(["has-session", "-t", name])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn start_tmux_session(
    tmux_session: &str,
    command_file: &Path,
    log_file: &Path,
    env: &[(String, String)],
) -> Result<(), String> {
    let tmux_command = format!("exec {}", shell_quote(&command_file.display().to_string()));
    let mut command = Command::new("tmux");
    command.args(["new-session", "-d", "-s", tmux_session]);
    for (key, value) in env {
        command.args(["-e", &format!("{key}={value}")]);
        command.env(key, value);
    }
    let status = command
        .arg(&tmux_command)
        .status()
        .map_err(|err| format!("failed to start tmux: {err}"))?;
    if !status.success() {
        return Err(format!(
            "tmux failed to start agent session '{}'",
            tmux_session
        ));
    }
    if let Some(parent) = log_file.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create log directory {}: {err}", parent.display()))?;
    }
    let pipe_command = format!("cat >> {}", shell_quote(&log_file.display().to_string()));
    let status = Command::new("tmux")
        .args(["pipe-pane", "-o", "-t", tmux_session, &pipe_command])
        .status()
        .map_err(|err| format!("failed to attach tmux log pipe: {err}"))?;
    if !status.success() {
        kill_tmux_session(tmux_session);
        return Err(format!(
            "tmux failed to attach log pipe for agent session '{}'",
            tmux_session
        ));
    }
    Ok(())
}

fn send_tmux_ctrl_c(tmux_session: &str) {
    let _ = Command::new("tmux")
        .args(["send-keys", "-t", tmux_session, "C-c"])
        .status();
}

fn kill_tmux_session(tmux_session: &str) {
    let _ = Command::new("tmux")
        .args(["kill-session", "-t", tmux_session])
        .status();
}

fn write_command_file(path: &Path, shell_command: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        path,
        format!("#!/usr/bin/env bash\nset -euo pipefail\n{shell_command}\n"),
    )?;
    make_executable(path)
}

#[cfg(unix)]
fn make_executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

fn append_operation_report(path: &Path, report: OperationReport) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let line = serde_json::to_string(&report)?;
    writeln!(file, "{line}")?;
    Ok(())
}

fn active_agent_file(ctx: &AgentContext) -> PathBuf {
    ctx.paths.runtime_dir.join("active.json")
}

fn read_active_agent(ctx: &AgentContext) -> Result<Option<ActiveAgent>, String> {
    let path = active_agent_file(ctx);
    if !path.exists() {
        return Ok(None);
    }
    let contents = std::fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    serde_json::from_str(&contents)
        .map(Some)
        .map_err(|err| format!("failed to parse {}: {err}", path.display()))
}

fn write_active_agent(ctx: &AgentContext, active: ActiveAgent) -> std::io::Result<()> {
    let path = active_agent_file(ctx);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        path,
        format!("{}\n", serde_json::to_string_pretty(&active)?),
    )
}

fn ensure_no_active_agent(ctx: &AgentContext) -> Result<(), String> {
    let Some(active) = read_active_agent(ctx)? else {
        return Ok(());
    };
    if tmux_session_exists(&active.tmux_session) {
        return Err(format!(
            "agent '{}' already has an active process in tmux session '{}'. Stop it before starting another prompt. Attach with: tmux attach -t {}",
            active.name, active.tmux_session, active.tmux_session
        ));
    }
    let _ = std::fs::remove_file(active_agent_file(ctx));
    Ok(())
}

fn failure_report(ctx: &AgentContext, options: &AgentStartOptions, error: &str) -> OperationReport {
    OperationReport {
        time: utc_now(),
        operation: "agent.start".to_string(),
        status: "failed".to_string(),
        agent_name: options.name.clone(),
        agent_kind: match options.kind {
            AgentKind::Claude => "claude",
            AgentKind::Codex => "codex",
        }
        .to_string(),
        agent_dir: agent_dir_from_context(ctx),
        tmux_session: options.tmux_session.clone(),
        attach_command: options
            .tmux_session
            .as_ref()
            .map(|session| format!("tmux attach -t {session}")),
        command_file: Some(ctx.paths.runtime_dir.join("start.sh")),
        log_file: Some(ctx.paths.log_file.clone()),
        events_file: agent_dir_from_context(ctx).join("events.jsonl"),
        error: Some(error.to_string()),
    }
}

fn send_failure_report(
    ctx: &AgentContext,
    options: &AgentSendOptions,
    error: &str,
) -> OperationReport {
    OperationReport {
        time: utc_now(),
        operation: "agent.send".to_string(),
        status: "failed".to_string(),
        agent_name: options.name.clone(),
        agent_kind: match options.kind {
            AgentKind::Claude => "claude",
            AgentKind::Codex => "codex",
        }
        .to_string(),
        agent_dir: agent_dir_from_context(ctx),
        tmux_session: options.tmux_session.clone(),
        attach_command: options
            .tmux_session
            .as_ref()
            .map(|session| format!("tmux attach -t {session}")),
        command_file: Some(ctx.paths.runtime_dir.join("send.sh")),
        log_file: Some(ctx.paths.log_file.clone()),
        events_file: agent_dir_from_context(ctx).join("events.jsonl"),
        error: Some(error.to_string()),
    }
}

fn stop_failure_report(
    ctx: &AgentContext,
    options: &AgentStopOptions,
    error: &str,
) -> OperationReport {
    OperationReport {
        time: utc_now(),
        operation: "agent.stop".to_string(),
        status: "failed".to_string(),
        agent_name: options.name.clone(),
        agent_kind: match options.kind {
            AgentKind::Claude => "claude",
            AgentKind::Codex => "codex",
        }
        .to_string(),
        agent_dir: agent_dir_from_context(ctx),
        tmux_session: options.tmux_session.clone(),
        attach_command: options
            .tmux_session
            .as_ref()
            .map(|session| format!("tmux attach -t {session}")),
        command_file: None,
        log_file: Some(ctx.paths.log_file.clone()),
        events_file: agent_dir_from_context(ctx).join("events.jsonl"),
        error: Some(error.to_string()),
    }
}

fn agent_dir_from_context(ctx: &AgentContext) -> PathBuf {
    ctx.paths
        .runtime_dir
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .to_path_buf()
}

fn utc_now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn command_spec_to_shell(command: &CommandSpec) -> String {
    let mut parts = Vec::new();
    parts.push("cd".to_string());
    parts.push(shell_quote(&command.cwd.display().to_string()));
    parts.push("&&".to_string());
    parts.push("exec".to_string());
    if !command.env.is_empty() {
        parts.push("env".to_string());
        for (key, value) in &command.env {
            if is_sensitive_env_key(key) {
                parts.push(format!("{key}=\"${{{key}:?set {key}}}\""));
            } else {
                parts.push(format!("{key}={}", shell_quote(value)));
            }
        }
    }
    parts.push(shell_quote(&command.program.display().to_string()));
    for arg in &command.args {
        parts.push(shell_quote(arg));
    }
    parts.join(" ")
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn sanitize_agent_name(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "agent".to_string()
    } else {
        sanitized
    }
}

fn is_sensitive_env_key(key: &str) -> bool {
    let upper = key.to_ascii_uppercase();
    upper.contains("TOKEN")
        || upper.contains("SECRET")
        || upper.contains("PASSWORD")
        || upper.contains("API_KEY")
}

fn absolutize_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_quotes_command_specs() {
        let command = CommandSpec {
            program: PathBuf::from("/bin/echo"),
            args: vec!["hello world".to_string(), "it's ok".to_string()],
            env: vec![("A".to_string(), "b c".to_string())],
            cwd: PathBuf::from("/tmp/a b"),
        };

        assert_eq!(
            command_spec_to_shell(&command),
            "cd '/tmp/a b' && exec env A='b c' '/bin/echo' 'hello world' 'it'\\''s ok'"
        );
    }

    #[test]
    fn command_files_do_not_persist_sensitive_env_values() {
        let command = CommandSpec {
            program: PathBuf::from("/bin/echo"),
            args: vec!["hi".to_string()],
            env: vec![
                (
                    "CLAUDE_CODE_OAUTH_TOKEN".to_string(),
                    "super-secret".to_string(),
                ),
                (
                    "WORLD_BASE_URL".to_string(),
                    "http://localhost:1".to_string(),
                ),
            ],
            cwd: PathBuf::from("/tmp"),
        };
        let shell = command_spec_to_shell(&command);

        assert!(!shell.contains("super-secret"));
        assert!(shell.contains(
            "CLAUDE_CODE_OAUTH_TOKEN=\"${CLAUDE_CODE_OAUTH_TOKEN:?set CLAUDE_CODE_OAUTH_TOKEN}\""
        ));
        assert!(shell.contains("WORLD_BASE_URL='http://localhost:1'"));
    }

    #[test]
    fn sanitizes_empty_and_unsafe_agent_names() {
        assert_eq!(sanitize_agent_name("mesa agent/1"), "mesa_agent_1");
        assert_eq!(sanitize_agent_name(""), "agent");
    }

    #[test]
    fn writes_rerunnable_command_files() {
        let root = test_root();
        let command_file = root.join("runtime/start.sh");

        write_command_file(&command_file, "cd '/tmp' && exec '/bin/echo' 'hi'").unwrap();

        let contents = std::fs::read_to_string(&command_file).unwrap();
        assert!(contents.starts_with("#!/usr/bin/env bash\nset -euo pipefail\n"));
        assert!(contents.contains("exec '/bin/echo' 'hi'"));
    }

    #[test]
    fn appends_operation_reports_as_jsonl() {
        let root = test_root();
        let events_file = root.join("events.jsonl");
        let report = OperationReport {
            time: "2026-05-17T00:00:00Z".to_string(),
            operation: "agent.start".to_string(),
            status: "started".to_string(),
            agent_name: "agent-0".to_string(),
            agent_kind: "claude".to_string(),
            agent_dir: root.clone(),
            tmux_session: Some("session".to_string()),
            attach_command: Some("tmux attach -t session".to_string()),
            command_file: Some(root.join("runtime/start.sh")),
            log_file: Some(root.join("logs/agent.log")),
            events_file: events_file.clone(),
            error: None,
        };

        append_operation_report(&events_file, report).unwrap();

        let contents = std::fs::read_to_string(&events_file).unwrap();
        let value: serde_json::Value = serde_json::from_str(contents.trim()).unwrap();
        assert_eq!(value["operation"], "agent.start");
        assert_eq!(value["status"], "started");
        assert_eq!(value["tmux_session"], "session");
    }

    #[test]
    fn writes_and_reads_active_agent_state() {
        let ctx = test_context(test_root(), AgentKind::Claude);
        ctx.prepare_paths().unwrap();
        let active = ActiveAgent {
            name: "agent-0".to_string(),
            kind: "claude".to_string(),
            operation: "agent.start".to_string(),
            tmux_session: "session".to_string(),
            command_file: ctx.paths.runtime_dir.join("start.sh"),
            started_at: "2026-05-17T00:00:00Z".to_string(),
        };

        write_active_agent(&ctx, active.clone()).unwrap();

        assert_eq!(read_active_agent(&ctx).unwrap(), Some(active));
    }

    #[test]
    fn clears_stale_active_agent_state() {
        let ctx = test_context(test_root(), AgentKind::Claude);
        ctx.prepare_paths().unwrap();
        write_active_agent(
            &ctx,
            ActiveAgent {
                name: "agent-0".to_string(),
                kind: "claude".to_string(),
                operation: "agent.start".to_string(),
                tmux_session: "definitely-not-running".to_string(),
                command_file: ctx.paths.runtime_dir.join("start.sh"),
                started_at: "2026-05-17T00:00:00Z".to_string(),
            },
        )
        .unwrap();

        ensure_no_active_agent(&ctx).unwrap();

        assert!(!active_agent_file(&ctx).exists());
    }

    fn test_root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "clawblox-agent-runner-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap()
        ))
    }

    fn test_context(root: PathBuf, kind: AgentKind) -> AgentContext {
        let kind_name = match kind {
            AgentKind::Claude => "claude",
            AgentKind::Codex => "codex",
        };
        AgentContext {
            paths: AgentPaths {
                workspace_dir: root.join("workspace"),
                prompt_file: root.join("runtime/system_prompt.md"),
                session_id_file: root.join(format!("{kind_name}_session_id.txt")),
                transcript_dir: root.join("session"),
                runtime_dir: root.join("runtime"),
                log_file: root.join("logs/agent.log"),
                agent_home_dir: root.join("home"),
            },
            settings: AgentSettings {
                model: "test-model".to_string(),
                permission_mode: "bypassPermissions".to_string(),
                initial_prompt: "Begin".to_string(),
                lifecycle_prompt: "Save".to_string(),
                auto_continue_prompt: "Continue".to_string(),
                extra_args: Vec::new(),
                env: Vec::new(),
                sandbox_enabled: false,
            },
        }
    }
}
