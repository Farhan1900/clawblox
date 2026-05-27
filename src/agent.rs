use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

use crate::agent_sandbox::SandboxLayout;

pub type AgentResult<T> = Result<T, AgentError>;

#[derive(Debug)]
pub enum AgentError {
    InvalidConfig(String),
    Io(std::io::Error),
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig(message) => write!(f, "invalid agent config: {message}"),
            Self::Io(err) => write!(f, "{err}"),
        }
    }
}

impl Error for AgentError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidConfig(_) => None,
            Self::Io(err) => Some(err),
        }
    }
}

impl From<std::io::Error> for AgentError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentPaths {
    pub workspace_dir: PathBuf,
    pub prompt_file: PathBuf,
    pub session_id_file: PathBuf,
    pub transcript_dir: PathBuf,
    pub runtime_dir: PathBuf,
    pub log_file: PathBuf,
    pub agent_home_dir: PathBuf,
}

impl AgentPaths {
    pub fn validate(&self) -> AgentResult<()> {
        require_nonempty_path("workspace_dir", &self.workspace_dir)?;
        require_nonempty_path("prompt_file", &self.prompt_file)?;
        require_nonempty_path("session_id_file", &self.session_id_file)?;
        require_nonempty_path("transcript_dir", &self.transcript_dir)?;
        require_nonempty_path("runtime_dir", &self.runtime_dir)?;
        require_nonempty_path("log_file", &self.log_file)?;
        require_nonempty_path("agent_home_dir", &self.agent_home_dir)?;
        Ok(())
    }

    pub fn create_dirs(&self) -> AgentResult<()> {
        self.validate()?;
        std::fs::create_dir_all(&self.workspace_dir)?;
        std::fs::create_dir_all(&self.transcript_dir)?;
        std::fs::create_dir_all(&self.runtime_dir)?;
        std::fs::create_dir_all(&self.agent_home_dir)?;
        create_parent_dir(&self.prompt_file)?;
        create_parent_dir(&self.session_id_file)?;
        create_parent_dir(&self.log_file)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentSettings {
    pub model: String,
    pub permission_mode: String,
    pub initial_prompt: String,
    pub lifecycle_prompt: String,
    pub auto_continue_prompt: String,
    pub extra_args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub sandbox_enabled: bool,
}

impl AgentSettings {
    pub fn validate(&self) -> AgentResult<()> {
        require_nonempty("model", &self.model)?;
        require_nonempty("permission_mode", &self.permission_mode)?;
        require_nonempty("initial_prompt", &self.initial_prompt)?;
        require_nonempty("lifecycle_prompt", &self.lifecycle_prompt)?;
        require_nonempty("auto_continue_prompt", &self.auto_continue_prompt)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentContext {
    pub paths: AgentPaths,
    pub settings: AgentSettings,
    pub sandbox: Option<SandboxLayout>,
}

impl AgentContext {
    pub fn validate(&self) -> AgentResult<()> {
        self.paths.validate()?;
        self.settings.validate()?;
        Ok(())
    }

    pub fn prepare_paths(&self) -> AgentResult<()> {
        self.paths.create_dirs()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cwd: PathBuf,
}

impl CommandSpec {
    pub fn new(program: impl Into<PathBuf>, cwd: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            env: Vec::new(),
            cwd: cwd.into(),
        }
    }

    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    pub fn envs(mut self, env: impl IntoIterator<Item = (String, String)>) -> Self {
        self.env.extend(env);
        self
    }

    pub fn with_extra_args(mut self, args: impl IntoIterator<Item = String>) -> Self {
        self.args.extend(args);
        self
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SandboxRequirements {
    pub read_only_paths: Vec<PathBuf>,
    pub writable_paths: Vec<PathBuf>,
    pub env_vars: Vec<String>,
    pub needs_network: bool,
}

pub trait Agent {
    fn name(&self) -> &'static str;

    fn check_setup(&self, ctx: &AgentContext) -> AgentResult<()>;

    fn start(&self, ctx: &AgentContext) -> AgentResult<CommandSpec>;

    fn continue_session(&self, ctx: &AgentContext, prompt: &str) -> AgentResult<CommandSpec>;

    fn configure_auto_continue(&self, ctx: &AgentContext) -> AgentResult<()>;

    fn collect_transcripts(&self, ctx: &AgentContext) -> AgentResult<()>;

    fn sandbox_requirements(&self, ctx: &AgentContext) -> AgentResult<SandboxRequirements>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaudeAgent {
    pub binary: PathBuf,
    pub bare: bool,
    pub disallowed_tools: Option<String>,
}

impl Default for ClaudeAgent {
    fn default() -> Self {
        Self {
            binary: PathBuf::from("claude"),
            bare: false,
            disallowed_tools: Some("WebFetch,WebSearch".to_string()),
        }
    }
}

impl Agent for ClaudeAgent {
    fn name(&self) -> &'static str {
        "claude"
    }

    fn check_setup(&self, ctx: &AgentContext) -> AgentResult<()> {
        ctx.validate()?;
        require_program(&self.binary, "claude CLI")?;
        Ok(())
    }

    fn start(&self, ctx: &AgentContext) -> AgentResult<CommandSpec> {
        ctx.validate()?;
        let session_id = read_or_create_session_id(&ctx.paths.session_id_file)?;
        Ok(self
            .base_command(ctx)
            .arg("--session-id")
            .arg(session_id)
            .arg(&ctx.settings.initial_prompt))
    }

    fn continue_session(&self, ctx: &AgentContext, prompt: &str) -> AgentResult<CommandSpec> {
        ctx.validate()?;
        require_nonempty("prompt", prompt)?;
        Ok(self.base_command(ctx).arg("--continue").arg(prompt))
    }

    fn configure_auto_continue(&self, ctx: &AgentContext) -> AgentResult<()> {
        ctx.validate()?;
        let plugin_dir = claude_plugin_dir(ctx);
        let bin_dir = plugin_dir.join("bin");
        let hooks_dir = plugin_dir.join("hooks");
        let manifest_dir = plugin_dir.join(".claude-plugin");
        std::fs::create_dir_all(&bin_dir)?;
        std::fs::create_dir_all(&hooks_dir)?;
        std::fs::create_dir_all(&manifest_dir)?;

        write_file(
            &manifest_dir.join("plugin.json"),
            r#"{"name":"clawblox-claude","version":"0.0.1","description":"Claude hooks for clawblox agents"}
"#,
        )?;
        write_file(&hooks_dir.join("hooks.json"), CLAUDE_HOOKS_JSON)?;
        write_executable_file(&bin_dir.join("session_start.sh"), CLAUDE_SESSION_START_SH)?;
        write_executable_file(&bin_dir.join("session_end.sh"), CLAUDE_SESSION_END_SH)?;
        write_executable_file(&bin_dir.join("stop.sh"), CLAUDE_STOP_SH)?;
        write_executable_file(
            &bin_dir.join("post_tool_use_chat.py"),
            CLAUDE_POST_TOOL_USE_CHAT_PY,
        )?;
        write_executable_file(
            &bin_dir.join("user_prompt_submit.sh"),
            "#!/usr/bin/env bash\nexit 0\n",
        )?;
        write_executable_file(
            &bin_dir.join("stop_failure.sh"),
            "#!/usr/bin/env bash\nexit 0\n",
        )?;
        write_executable_file(
            &bin_dir.join("notification.sh"),
            "#!/usr/bin/env bash\nexit 0\n",
        )?;
        if let Some(layout) = &ctx.sandbox {
            prepare_claude_sandbox_home(ctx, layout)?;
            write_claude_sandbox_settings(ctx)?;
            write_sandbox_hosts_file(ctx)?;
        }
        Ok(())
    }

    fn collect_transcripts(&self, ctx: &AgentContext) -> AgentResult<()> {
        collect_claude_transcript(ctx)
    }

    fn sandbox_requirements(&self, ctx: &AgentContext) -> AgentResult<SandboxRequirements> {
        ctx.validate()?;
        Ok(SandboxRequirements {
            read_only_paths: vec![
                ctx.paths.prompt_file.clone(),
                claude_plugin_dir(ctx),
                self.binary.clone(),
            ],
            writable_paths: vec![
                ctx.paths.workspace_dir.clone(),
                ctx.paths.agent_home_dir.clone(),
                ctx.paths.runtime_dir.clone(),
                ctx.paths.transcript_dir.clone(),
                ctx.paths
                    .log_file
                    .parent()
                    .unwrap_or_else(|| Path::new(""))
                    .to_path_buf(),
            ],
            env_vars: vec![
                "HOME".to_string(),
                "PATH".to_string(),
                "CLAWBLOX_CLAUDE_AUTO_LOOP".to_string(),
                "CLAUDE_AUTO_LOOP_MESSAGE".to_string(),
                "CLAWBLOX_CLAUDE_RUNTIME_DIR".to_string(),
                "WORLD_INTERNAL_BASE_URL".to_string(),
            ],
            needs_network: ctx.settings.sandbox_enabled,
        })
    }
}

impl ClaudeAgent {
    fn base_command(&self, ctx: &AgentContext) -> CommandSpec {
        let sandbox = ctx.sandbox.as_ref();
        let program = sandbox
            .map(|layout| layout.guest_binary.clone())
            .unwrap_or_else(|| self.binary.clone());
        let cwd = sandbox
            .map(|layout| layout.guest_workspace.clone())
            .unwrap_or_else(|| ctx.paths.workspace_dir.clone());
        let plugin_dir = sandbox
            .and_then(|layout| layout.guest_plugin.clone())
            .unwrap_or_else(|| claude_plugin_dir(ctx));
        let prompt_file = sandbox
            .map(|layout| layout.guest_prompt.clone())
            .unwrap_or_else(|| ctx.paths.prompt_file.clone());
        let log_dir = sandbox
            .map(|layout| layout.guest_logs.clone())
            .unwrap_or_else(|| {
                ctx.paths
                    .log_file
                    .parent()
                    .unwrap_or_else(|| Path::new(""))
                    .to_path_buf()
            });
        let metadata_file = sandbox
            .map(|layout| layout.guest_runtime.join("hook_session.env"))
            .unwrap_or_else(|| claude_hook_metadata_file(ctx));
        let runtime_dir = sandbox
            .map(|layout| layout.guest_runtime.clone())
            .unwrap_or_else(|| ctx.paths.runtime_dir.clone());
        let agent_dir = sandbox
            .map(|layout| layout.guest_workspace.clone())
            .unwrap_or_else(|| ctx.paths.agent_home_dir.clone());

        let mut command = CommandSpec::new(program, cwd)
            .arg("--plugin-dir")
            .arg(plugin_dir.display().to_string())
            .arg("--append-system-prompt-file")
            .arg(prompt_file.display().to_string())
            .arg("--permission-mode")
            .arg(&ctx.settings.permission_mode)
            .arg("--model")
            .arg(&ctx.settings.model)
            .envs(ctx.settings.env.clone())
            .env("CLAWBLOX_CLAUDE_AGENT_DIR", agent_dir.display().to_string())
            .env("CLAWBLOX_CLAUDE_LOG_DIR", log_dir.display().to_string())
            .env(
                "CLAWBLOX_CLAUDE_METADATA_FILE",
                metadata_file.display().to_string(),
            )
            .env("CLAWBLOX_CLAUDE_RUNTIME_DIR", runtime_dir.display().to_string())
            .env("CLAWBLOX_CLAUDE_AUTO_LOOP", "1")
            .env(
                "CLAUDE_AUTO_LOOP_MESSAGE",
                &ctx.settings.auto_continue_prompt,
            );

        if ctx.sandbox.is_some() {
            command = command
                .arg("--settings")
                .arg("/runtime/claude_sandbox_settings.json");
            if let Some(disallowed_tools) = &self.disallowed_tools {
                if !disallowed_tools.trim().is_empty() {
                    command = command.arg("--disallowedTools").arg(disallowed_tools);
                }
            }
        }

        if self.bare {
            command.args.insert(0, "--bare".to_string());
        }

        command.args.extend(ctx.settings.extra_args.clone());
        command
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexAgent {
    pub binary: PathBuf,
}

impl Default for CodexAgent {
    fn default() -> Self {
        Self {
            binary: PathBuf::from("codex"),
        }
    }
}

impl Agent for CodexAgent {
    fn name(&self) -> &'static str {
        "codex"
    }

    fn check_setup(&self, ctx: &AgentContext) -> AgentResult<()> {
        ctx.validate()?;
        require_program(&self.binary, "codex CLI")?;
        Ok(())
    }

    fn start(&self, ctx: &AgentContext) -> AgentResult<CommandSpec> {
        ctx.validate()?;
        Ok(self.base_command(ctx).arg(&ctx.settings.initial_prompt))
    }

    fn continue_session(&self, ctx: &AgentContext, prompt: &str) -> AgentResult<CommandSpec> {
        ctx.validate()?;
        require_nonempty("prompt", prompt)?;
        let session_id = read_required_session_id(&ctx.paths.session_id_file)?;
        Ok(self
            .base_resume_command(ctx)
            .arg(session_id)
            .arg(prompt.to_string()))
    }

    fn configure_auto_continue(&self, ctx: &AgentContext) -> AgentResult<()> {
        ctx.validate()?;
        if let Some(layout) = &ctx.sandbox {
            prepare_codex_sandbox_home(layout)?;
            write_sandbox_hosts_file(ctx)?;
        }
        prepare_codex_home(ctx)?;
        let stop_hook_command = if ctx.sandbox.is_some() {
            "python3 /home/agent/.codex/stop_hook.py".to_string()
        } else {
            format!("python3 {}", codex_stop_hook_path(ctx).display())
        };
        let hooks_json = serde_json::json!({
            "hooks": {
                "Stop": [
                    {
                        "hooks": [
                            {
                                "type": "command",
                                "command": stop_hook_command,
                            }
                        ]
                    }
                ]
            }
        });
        write_executable_file(&codex_stop_hook_path(ctx), CODEX_STOP_HOOK_PY)?;
        write_file(
            &codex_hooks_path(ctx),
            &format!(
                "{}\n",
                serde_json::to_string_pretty(&hooks_json)
                    .map_err(|err| AgentError::InvalidConfig(err.to_string()))?
            ),
        )?;
        Ok(())
    }

    fn collect_transcripts(&self, ctx: &AgentContext) -> AgentResult<()> {
        collect_codex_transcript(ctx)
    }

    fn sandbox_requirements(&self, ctx: &AgentContext) -> AgentResult<SandboxRequirements> {
        ctx.validate()?;
        Ok(SandboxRequirements {
            read_only_paths: vec![ctx.paths.prompt_file.clone(), self.binary.clone()],
            writable_paths: vec![
                ctx.paths.workspace_dir.clone(),
                ctx.paths.agent_home_dir.clone(),
                ctx.paths.runtime_dir.clone(),
                ctx.paths.transcript_dir.clone(),
                ctx.paths
                    .log_file
                    .parent()
                    .unwrap_or_else(|| Path::new(""))
                    .to_path_buf(),
            ],
            env_vars: vec![
                "HOME".to_string(),
                "PATH".to_string(),
                "CLAWBLOX_CODEX_AUTO_LOOP".to_string(),
                "CODEX_AUTO_LOOP_MESSAGE".to_string(),
            ],
            needs_network: ctx.settings.sandbox_enabled,
        })
    }
}

impl CodexAgent {
    fn base_command(&self, ctx: &AgentContext) -> CommandSpec {
        let sandbox = ctx.sandbox.as_ref();
        let sandbox_mode = if ctx.sandbox.is_some() {
            "workspace-write"
        } else {
            "danger-full-access"
        };
        let program = sandbox
            .and_then(|layout| layout.codex_guest_node())
            .or_else(|| sandbox.map(|layout| layout.codex_guest_program()))
            .unwrap_or_else(|| self.binary.clone());
        let codex_program_arg = sandbox.and_then(|layout| {
            layout
                .codex_guest_node()
                .map(|_| layout.codex_guest_program().display().to_string())
        });
        let cwd = sandbox
            .map(|layout| layout.guest_workspace.clone())
            .unwrap_or_else(|| ctx.paths.workspace_dir.clone());
        let effective_workspace = sandbox
            .map(|layout| layout.guest_workspace.clone())
            .unwrap_or_else(|| ctx.paths.workspace_dir.clone());
        let home = sandbox
            .map(|layout| layout.guest_home.clone())
            .unwrap_or_else(|| ctx.paths.agent_home_dir.clone());
        let runtime_dir = sandbox
            .map(|layout| layout.guest_runtime.clone())
            .unwrap_or_else(|| ctx.paths.runtime_dir.clone());
        let log_dir = sandbox
            .map(|layout| layout.guest_logs.clone())
            .unwrap_or_else(|| {
                ctx.paths
                    .log_file
                    .parent()
                    .unwrap_or_else(|| Path::new(""))
                    .to_path_buf()
            });
        let mut command = CommandSpec::new(program, cwd);
        if let Some(codex_program_arg) = codex_program_arg {
            command = command.arg(codex_program_arg);
        }
        command
            .arg("--no-alt-screen")
            .arg("-C")
            .arg(effective_workspace.display().to_string())
            .arg("-m")
            .arg(&ctx.settings.model)
            .arg("-s")
            .arg(sandbox_mode)
            .arg("-a")
            .arg(codex_approval_policy(&ctx.settings.permission_mode))
            .envs(ctx.settings.env.clone())
            .env("HOME", home.display().to_string())
            .env(
                "CLAWBLOX_CODEX_RUNTIME_DIR",
                runtime_dir.display().to_string(),
            )
            .env("CLAWBLOX_CODEX_LOG_DIR", log_dir.display().to_string())
            .env("CLAWBLOX_CODEX_AUTO_LOOP", "1")
            .env(
                "CODEX_AUTO_LOOP_MESSAGE",
                &ctx.settings.auto_continue_prompt,
            )
            .with_extra_args(ctx.settings.extra_args.clone())
    }

    fn base_resume_command(&self, ctx: &AgentContext) -> CommandSpec {
        let mut command = self.base_command(ctx);
        command.args.insert(0, "resume".to_string());
        command
    }
}

fn create_parent_dir(path: &Path) -> AgentResult<()> {
    let Some(parent) = path.parent() else {
        return Err(AgentError::InvalidConfig(format!(
            "{} has no parent directory",
            path.display()
        )));
    };
    std::fs::create_dir_all(parent)?;
    Ok(())
}

fn write_file(path: &Path, contents: &str) -> AgentResult<()> {
    create_parent_dir(path)?;
    std::fs::write(path, contents)?;
    Ok(())
}

fn write_executable_file(path: &Path, contents: &str) -> AgentResult<()> {
    write_file(path, contents)?;
    make_executable(path)?;
    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> AgentResult<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> AgentResult<()> {
    Ok(())
}

fn require_program(path: &Path, label: &str) -> AgentResult<()> {
    if path.components().count() > 1 || path.is_absolute() {
        if path.exists() {
            return Ok(());
        }
        return Err(AgentError::InvalidConfig(format!(
            "{label} was not found at {}",
            path.display()
        )));
    }

    let Some(paths) = std::env::var_os("PATH") else {
        return Err(AgentError::InvalidConfig(format!(
            "{label} was not found in PATH"
        )));
    };
    for dir in std::env::split_paths(&paths) {
        if dir.join(path).exists() {
            return Ok(());
        }
    }
    Err(AgentError::InvalidConfig(format!(
        "{label} was not found in PATH"
    )))
}

fn read_or_create_session_id(path: &Path) -> AgentResult<String> {
    if let Ok(existing) = std::fs::read_to_string(path) {
        let trimmed = existing.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }
    create_parent_dir(path)?;
    let id = uuid::Uuid::new_v4().to_string();
    std::fs::write(path, format!("{id}\n"))?;
    Ok(id)
}

fn read_required_session_id(path: &Path) -> AgentResult<String> {
    let id = std::fs::read_to_string(path)
        .map_err(AgentError::Io)?
        .trim()
        .to_string();
    require_nonempty("session_id", &id)?;
    Ok(id)
}

fn claude_plugin_dir(ctx: &AgentContext) -> PathBuf {
    ctx.paths.runtime_dir.join("claude-plugin")
}

fn claude_hook_metadata_file(ctx: &AgentContext) -> PathBuf {
    ctx.paths.runtime_dir.join("hook_session.env")
}

fn codex_home_dir(ctx: &AgentContext) -> PathBuf {
    agent_home_dir(ctx).join(".codex")
}

fn agent_home_dir(ctx: &AgentContext) -> PathBuf {
    if ctx.settings.sandbox_enabled {
        sandbox_home_dir(ctx)
    } else {
        ctx.paths.agent_home_dir.clone()
    }
}

fn sandbox_home_dir(ctx: &AgentContext) -> PathBuf {
    agent_dir_from_context(ctx).join("sandbox-home")
}

fn agent_dir_from_context(ctx: &AgentContext) -> PathBuf {
    ctx.paths
        .runtime_dir
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .to_path_buf()
}

fn codex_history_file(ctx: &AgentContext) -> PathBuf {
    codex_home_dir(ctx).join("history.jsonl")
}

fn codex_stop_hook_path(ctx: &AgentContext) -> PathBuf {
    codex_home_dir(ctx).join("stop_hook.py")
}

fn codex_hooks_path(ctx: &AgentContext) -> PathBuf {
    codex_home_dir(ctx).join("hooks.json")
}

fn prepare_codex_home(ctx: &AgentContext) -> AgentResult<()> {
    let home = codex_home_dir(ctx);
    std::fs::create_dir_all(home.join("log"))?;
    std::fs::create_dir_all(home.join("sessions"))?;
    std::fs::create_dir_all(home.join("tmp"))?;
    sync_codex_user_state(ctx)?;
    write_codex_config(ctx)
}

fn sync_codex_user_state(ctx: &AgentContext) -> AgentResult<()> {
    let Some(user_home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Ok(());
    };
    let source = user_home.join(".codex");
    if !source.is_dir() {
        return Ok(());
    }
    let target = codex_home_dir(ctx);
    sync_codex_user_entry(
        &source.join("auth.json"),
        &target.join("auth.json"),
        ctx.settings.sandbox_enabled,
    )?;
    sync_codex_user_entry(
        &source.join("skills"),
        &target.join("skills"),
        ctx.settings.sandbox_enabled,
    )?;
    sync_codex_user_entry(
        &source.join("rules"),
        &target.join("rules"),
        ctx.settings.sandbox_enabled,
    )?;
    Ok(())
}

fn sync_codex_user_entry(source: &Path, target: &Path, copy: bool) -> AgentResult<()> {
    if !source.exists() || target.exists() {
        return Ok(());
    }
    create_parent_dir(target)?;
    if copy {
        copy_path(source, target)?;
    } else {
        symlink_path(source, target)?;
    }
    Ok(())
}

fn copy_path(source: &Path, target: &Path) -> AgentResult<()> {
    if source.is_dir() {
        std::fs::create_dir_all(target)?;
        for entry in std::fs::read_dir(source)? {
            let entry = entry?;
            copy_path(&entry.path(), &target.join(entry.file_name()))?;
        }
    } else {
        std::fs::copy(source, target)?;
    }
    Ok(())
}

fn prepare_claude_sandbox_home(ctx: &AgentContext, layout: &SandboxLayout) -> AgentResult<()> {
    let claude_dir = layout.host_home_dir.join(".claude");
    for rel in [
        ".local/bin",
        ".claude/projects",
        ".claude/session-env",
        ".claude/sessions",
        ".claude/plugins",
        ".claude/cache",
        ".claude/backups",
        ".claude/downloads",
        ".claude/file-history",
        ".claude/paste-cache",
        ".claude/plans",
        ".claude/shell-snapshots",
    ] {
        std::fs::create_dir_all(layout.host_home_dir.join(rel))?;
    }
    symlink_or_copy_file(
        &layout.guest_binary,
        &layout.host_home_dir.join(".local/bin/claude"),
    )?;

    let host_claude_dir = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".claude")));
    if let Some(token) = std::env::var_os("ANTHROPIC_OAUTH_TOKEN") {
        let token = token.to_string_lossy();
        write_file(
            &claude_dir.join(".credentials.json"),
            &format!(
                "{{\"claudeAiOauth\":{{\"accessToken\":{}}}}}\n",
                serde_json::to_string(&token)
                    .map_err(|err| AgentError::InvalidConfig(err.to_string()))?
            ),
        )?;
    } else if std::env::var_os("CLAUDE_CODE_OAUTH_TOKEN").is_some() {
        let _ = std::fs::remove_file(claude_dir.join(".credentials.json"));
    } else if let Some(host_dir) = &host_claude_dir {
        copy_if_exists(
            &host_dir.join(".credentials.json"),
            &claude_dir.join(".credentials.json"),
        )?;
    }
    if let Some(host_dir) = &host_claude_dir {
        copy_if_exists(
            &host_dir.join("settings.json"),
            &claude_dir.join("settings.json"),
        )?;
        copy_if_exists(
            &host_dir.join(".claude.json"),
            &layout.host_home_dir.join(".claude.json"),
        )?;
    }
    write_claude_workspace_trust(ctx, layout)?;
    Ok(())
}

fn write_claude_workspace_trust(_ctx: &AgentContext, layout: &SandboxLayout) -> AgentResult<()> {
    let path = layout.host_home_dir.join(".claude.json");
    let mut data = if path.is_file() {
        serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(&path)?)
            .unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };
    let Some(obj) = data.as_object_mut() else {
        data = serde_json::json!({});
        let obj = data.as_object_mut().expect("object");
        obj.insert("projects".to_string(), serde_json::json!({}));
        return write_json_file(&path, &data);
    };
    let projects = obj
        .entry("projects".to_string())
        .or_insert_with(|| serde_json::json!({}));
    let project_obj = projects.as_object_mut().ok_or_else(|| {
        AgentError::InvalidConfig("Claude projects config is not an object".into())
    })?;
    project_obj.insert(
        "/workspace".to_string(),
        serde_json::json!({
            "hasTrustDialogAccepted": true,
            "allowedTools": [],
            "mcpContextUris": [],
            "mcpServers": {},
            "enabledMcpjsonServers": [],
            "disabledMcpjsonServers": [],
        }),
    );
    obj.insert(
        "hasCompletedOnboarding".to_string(),
        serde_json::json!(true),
    );
    obj.entry("lastOnboardingVersion".to_string())
        .or_insert_with(|| serde_json::json!("2.1.116"));
    write_json_file(&path, &data)
}

fn write_claude_sandbox_settings(ctx: &AgentContext) -> AgentResult<()> {
    let world_base_url = env_value(ctx, "WORLD_BASE_URL").unwrap_or_default();
    let host = parse_http_host(&world_base_url).unwrap_or_default();
    let mut allowed_domains = Vec::new();
    if !host.is_empty() && !matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1") {
        allowed_domains.push(host);
    }
    let mut network = serde_json::json!({
        "allowedDomains": allowed_domains,
        "deniedDomains": ["localhost", "127.0.0.1"],
    });
    if let Some(port) = world_proxy_port(ctx) {
        network["httpProxyPort"] = serde_json::json!(port);
    }
    let settings = serde_json::json!({
        "sandbox": {
            "enabled": true,
            "failIfUnavailable": true,
            "allowUnsandboxedCommands": false,
            "autoAllowBashIfSandboxed": true,
            "network": network,
        }
    });
    write_json_file(
        &ctx.paths.runtime_dir.join("claude_sandbox_settings.json"),
        &settings,
    )
}

fn prepare_codex_sandbox_home(layout: &SandboxLayout) -> AgentResult<()> {
    for rel in [
        ".local/bin",
        ".codex/log",
        ".codex/sessions",
        ".codex/tmp",
        ".codex/cache",
        ".codex/memories",
    ] {
        std::fs::create_dir_all(layout.host_home_dir.join(rel))?;
    }
    symlink_or_copy_file(
        &layout.guest_binary,
        &layout.host_home_dir.join(".local/bin/codex"),
    )
}

fn write_sandbox_hosts_file(ctx: &AgentContext) -> AgentResult<()> {
    let path = ctx.paths.runtime_dir.join("sandbox_hosts");
    let mut hosts = "127.0.0.1 localhost\n::1 localhost ip6-localhost ip6-loopback\n".to_string();
    if let Some(host) = env_value(ctx, "WORLD_BASE_URL").and_then(|url| parse_http_host(&url)) {
        if !matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1") && !hosts.contains(&host) {
            hosts = format!("{}\n127.0.0.1 {host}\n", hosts.trim_end());
        }
    }
    write_file(&path, &hosts)
}

fn copy_if_exists(source: &Path, target: &Path) -> AgentResult<()> {
    if source.exists() {
        copy_path(source, target)?;
    }
    Ok(())
}

#[cfg(unix)]
fn symlink_or_copy_file(source: &Path, target: &Path) -> AgentResult<()> {
    create_parent_dir(target)?;
    let _ = std::fs::remove_file(target);
    std::os::unix::fs::symlink(source, target)?;
    Ok(())
}

#[cfg(not(unix))]
fn symlink_or_copy_file(source: &Path, target: &Path) -> AgentResult<()> {
    create_parent_dir(target)?;
    std::fs::copy(source, target)?;
    Ok(())
}

fn write_json_file(path: &Path, value: &serde_json::Value) -> AgentResult<()> {
    write_file(
        path,
        &format!(
            "{}\n",
            serde_json::to_string_pretty(value)
                .map_err(|err| AgentError::InvalidConfig(err.to_string()))?
        ),
    )
}

fn env_value(ctx: &AgentContext, key: &str) -> Option<String> {
    ctx.settings
        .env
        .iter()
        .rev()
        .find_map(|(name, value)| (name == key).then(|| value.clone()))
}

fn world_proxy_port(ctx: &AgentContext) -> Option<u16> {
    env_value(ctx, "CLAWBLOX_WORLD_HTTP_PROXY_PORT").and_then(|value| value.parse().ok())
}

fn parse_http_host(url: &str) -> Option<String> {
    let without_scheme = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))?;
    let authority = without_scheme.split('/').next().unwrap_or("");
    let host = authority
        .rsplit('@')
        .next()
        .unwrap_or(authority)
        .split(':')
        .next()
        .unwrap_or("")
        .trim();
    (!host.is_empty()).then(|| host.to_string())
}

#[cfg(unix)]
fn symlink_path(source: &Path, target: &Path) -> AgentResult<()> {
    std::os::unix::fs::symlink(source, target)?;
    Ok(())
}

#[cfg(windows)]
fn symlink_path(source: &Path, target: &Path) -> AgentResult<()> {
    if source.is_dir() {
        std::os::windows::fs::symlink_dir(source, target)?;
    } else {
        std::os::windows::fs::symlink_file(source, target)?;
    }
    Ok(())
}

fn write_codex_config(ctx: &AgentContext) -> AgentResult<()> {
    let mut config = String::new();
    config.push_str("developer_instructions = ");
    config.push_str(
        &serde_json::to_string(
            &std::fs::read_to_string(&ctx.paths.prompt_file).unwrap_or_default(),
        )
        .map_err(|err| AgentError::InvalidConfig(err.to_string()))?,
    );
    if let Some(port) = ctx.sandbox.as_ref().and_then(|_| world_proxy_port(ctx)) {
        config.push_str(
            "\ndefault_permissions = \"workspace\"\n\n\
             [sandbox_workspace_write]\n\
             writable_roots = [\"/workspace\"]\n\
             network_access = true\n\n\
             [permissions.workspace.network]\n\
             enabled = true\n",
        );
        config.push_str(&format!("proxy_url = \"http://127.0.0.1:{port}\"\n"));
        config.push_str(
            "enable_socks5 = false\n\
             allow_upstream_proxy = false\n\
             allow_local_binding = true\n\n\
             [permissions.workspace.network.domains]\n\
             \"world\" = \"allow\"\n\n",
        );
    }
    config.push_str("\n[features]\nhooks = true\n");
    if ctx.sandbox.is_some() && world_proxy_port(ctx).is_some() {
        config.push_str("network_proxy = true\n");
    }
    config.push('\n');
    let project_workspace = ctx
        .sandbox
        .as_ref()
        .map(|layout| layout.guest_workspace.display().to_string())
        .unwrap_or_else(|| ctx.paths.workspace_dir.display().to_string());
    config.push_str(&format!(
        "[projects.{}]\ntrust_level = \"trusted\"\n",
        serde_json::to_string(&project_workspace)
            .map_err(|err| AgentError::InvalidConfig(err.to_string()))?
    ));
    write_file(&codex_home_dir(ctx).join("config.toml"), &config)
}

fn codex_approval_policy(input: &str) -> String {
    match input {
        "bypassPermissions" | "bypass" | "never" => "never".to_string(),
        "acceptEdits" | "default" => "on-request".to_string(),
        other => other.to_string(),
    }
}

fn collect_claude_transcript(ctx: &AgentContext) -> AgentResult<()> {
    let metadata = claude_hook_metadata_file(ctx);
    if !metadata.exists() {
        return Ok(());
    }

    let mut session_id = String::new();
    let mut transcript_path = String::new();
    for line in std::fs::read_to_string(&metadata)?.lines() {
        if let Some(value) = line.strip_prefix("SESSION_ID=") {
            session_id = unquote_shell_value(value);
        } else if let Some(value) = line.strip_prefix("TRANSCRIPT_PATH=") {
            transcript_path = unquote_shell_value(value);
        }
    }

    if !session_id.trim().is_empty() {
        create_parent_dir(&ctx.paths.session_id_file)?;
        std::fs::write(
            &ctx.paths.session_id_file,
            format!("{}\n", session_id.trim()),
        )?;
    }

    if transcript_path.trim().is_empty() {
        return Ok(());
    }
    let src = map_sandbox_path(ctx, transcript_path.trim());
    if src.is_file() {
        std::fs::create_dir_all(&ctx.paths.transcript_dir)?;
        let dst = ctx.paths.transcript_dir.join(
            src.file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("transcript.jsonl")),
        );
        std::fs::copy(src, dst)?;
    }
    Ok(())
}

fn collect_codex_transcript(ctx: &AgentContext) -> AgentResult<()> {
    let history = codex_history_file(ctx);
    if !history.exists() {
        return Ok(());
    }

    let mut latest_session_id = String::new();
    for raw in std::fs::read_to_string(&history)?.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(raw.trim()) else {
            continue;
        };
        if let Some(session_id) = value.get("session_id").and_then(|value| value.as_str()) {
            if !session_id.trim().is_empty() {
                latest_session_id = session_id.trim().to_string();
            }
        }
    }

    if latest_session_id.is_empty() {
        return Ok(());
    }

    create_parent_dir(&ctx.paths.session_id_file)?;
    std::fs::write(&ctx.paths.session_id_file, format!("{latest_session_id}\n"))?;

    let sessions_dir = codex_home_dir(ctx).join("sessions");
    if let Some(transcript) = find_latest_matching_jsonl(&sessions_dir, &latest_session_id)? {
        std::fs::create_dir_all(&ctx.paths.transcript_dir)?;
        std::fs::copy(
            transcript,
            ctx.paths
                .transcript_dir
                .join(format!("{latest_session_id}.jsonl")),
        )?;
    }
    Ok(())
}

fn map_sandbox_path(ctx: &AgentContext, path: &str) -> PathBuf {
    if ctx.settings.sandbox_enabled {
        if let Some(rel) = path.strip_prefix("/home/agent/") {
            return sandbox_home_dir(ctx).join(rel);
        }
        if let Some(rel) = path.strip_prefix("/workspace/") {
            return ctx.paths.workspace_dir.join(rel);
        }
        if let Some(rel) = path.strip_prefix("/runtime/") {
            return ctx.paths.runtime_dir.join(rel);
        }
        if let Some(rel) = path.strip_prefix("/logs/") {
            return ctx
                .paths
                .log_file
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .join(rel);
        }
    }
    PathBuf::from(path)
}

fn find_latest_matching_jsonl(root: &Path, needle: &str) -> AgentResult<Option<PathBuf>> {
    let mut matches = Vec::new();
    collect_matching_jsonl(root, needle, &mut matches)?;
    matches.sort();
    Ok(matches.pop())
}

fn collect_matching_jsonl(
    root: &Path,
    needle: &str,
    matches: &mut Vec<PathBuf>,
) -> AgentResult<()> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Ok(());
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_matching_jsonl(&path, needle, matches)?;
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.contains(needle) && name.ends_with(".jsonl"))
        {
            matches.push(path);
        }
    }
    Ok(())
}

fn unquote_shell_value(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() >= 2 {
        let bytes = trimmed.as_bytes();
        if (bytes[0] == b'\'' && bytes[trimmed.len() - 1] == b'\'')
            || (bytes[0] == b'"' && bytes[trimmed.len() - 1] == b'"')
        {
            return trimmed[1..trimmed.len() - 1].to_string();
        }
    }
    trimmed.to_string()
}

const CLAUDE_HOOKS_JSON: &str = r#"{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "\"${CLAUDE_PLUGIN_ROOT}/bin/session_start.sh\""
          }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "\"${CLAUDE_PLUGIN_ROOT}/bin/user_prompt_submit.sh\""
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "\"${CLAUDE_PLUGIN_ROOT}/bin/post_tool_use_chat.py\""
          }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "\"${CLAUDE_PLUGIN_ROOT}/bin/stop.sh\""
          }
        ]
      }
    ],
    "StopFailure": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "\"${CLAUDE_PLUGIN_ROOT}/bin/stop_failure.sh\""
          }
        ]
      }
    ],
    "Notification": [
      {
        "matcher": "idle_prompt",
        "hooks": [
          {
            "type": "command",
            "command": "\"${CLAUDE_PLUGIN_ROOT}/bin/notification.sh\""
          }
        ]
      }
    ],
    "SessionEnd": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "\"${CLAUDE_PLUGIN_ROOT}/bin/session_end.sh\""
          }
        ]
      }
    ]
  }
}
"#;

const CLAUDE_SESSION_START_SH: &str = r#"#!/usr/bin/env bash
set -euo pipefail

input="$(cat)"
session_id="$(INPUT_JSON="$input" node -e 'var data = JSON.parse(process.env.INPUT_JSON || "{}"); process.stdout.write(String(data.session_id || ""));')"
transcript_path="$(INPUT_JSON="$input" node -e 'var data = JSON.parse(process.env.INPUT_JSON || "{}"); process.stdout.write(String(data.transcript_path || ""));')"
log_dir="${CLAWBLOX_CLAUDE_LOG_DIR:-}"
metadata_file="${CLAWBLOX_CLAUDE_METADATA_FILE:-}"

if [[ -n "$log_dir" ]]; then
  mkdir -p "$log_dir"
  printf '%s\n' "$input" >"$log_dir/session_start.json"
  if [[ -n "$session_id" ]]; then
    printf '%s\n' "$input" >"$log_dir/session_start.${session_id}.json"
  fi
fi

if [[ -n "$metadata_file" ]]; then
  mkdir -p "$(dirname "$metadata_file")"
  {
    printf 'SESSION_ID=%q\n' "$session_id"
    printf 'TRANSCRIPT_PATH=%q\n' "$transcript_path"
  } >"$metadata_file"
fi
"#;

const CLAUDE_SESSION_END_SH: &str = r#"#!/usr/bin/env bash
set -euo pipefail

input="$(cat)"
session_id="$(INPUT_JSON="$input" node -e 'var data = JSON.parse(process.env.INPUT_JSON || "{}"); process.stdout.write(String(data.session_id || ""));')"
log_dir="${CLAWBLOX_CLAUDE_LOG_DIR:-}"

if [[ -n "$log_dir" ]]; then
  mkdir -p "$log_dir"
  printf '%s\n' "$input" >"$log_dir/session_end.json"
  if [[ -n "$session_id" ]]; then
    printf '%s\n' "$input" >"$log_dir/session_end.${session_id}.json"
  fi
fi
"#;

const CLAUDE_STOP_SH: &str = r#"#!/usr/bin/env bash
set -euo pipefail

input="$(cat)"
log_dir="${CLAWBLOX_CLAUDE_LOG_DIR:-}"
session_id="$(INPUT_JSON="$input" node -e 'var data = JSON.parse(process.env.INPUT_JSON || "{}"); process.stdout.write(String(data.session_id || ""));')"

if [[ -n "$log_dir" ]]; then
  mkdir -p "$log_dir"
  printf '%s\n' "$input" >"$log_dir/stop.json"
  if [[ -n "$session_id" ]]; then
    printf '%s\n' "$input" >"$log_dir/stop.${session_id}.json"
  fi
fi

auto_loop_message="${CLAUDE_AUTO_LOOP_MESSAGE:-}"
if [[ -z "$auto_loop_message" ]]; then
  auto_loop_message="Please don't stop. Just continue doing whatever you want to do."
fi

if [[ "${CLAWBLOX_CLAUDE_AUTO_LOOP:-0}" == "1" ]]; then
  CLAUDE_AUTO_LOOP_MESSAGE="$auto_loop_message" node -e 'process.stdout.write(JSON.stringify({ decision: "block", reason: process.env.CLAUDE_AUTO_LOOP_MESSAGE }))'
fi
"#;

const CLAUDE_POST_TOOL_USE_CHAT_PY: &str = r#"#!/usr/bin/env python3
import json
import os
import sys
from pathlib import Path
from urllib.error import URLError
from urllib.parse import urlencode
from urllib.request import Request, urlopen


CHAT_MESSAGES_LIMIT = 20


def normalize_name(value):
    return str(value or "").strip().lower()


def truncate_chat_content(value):
    trimmed = str(value or "").strip()
    if len(trimmed) <= 80:
        return trimmed
    return trimmed[:77] + "..."


def format_self_chat_feedback(message, world_agent_name):
    content = str(message.get("content") or "").strip()
    if not content:
        return None

    audibility = message.get("audibility")
    if not isinstance(audibility, list):
        audibility = []

    hearers = []
    for entry in audibility:
        if not isinstance(entry, dict) or not entry.get("heard"):
            continue
        name = str(entry.get("agent_name") or "").strip()
        if name and normalize_name(name) != world_agent_name:
            hearers.append(name)
    unique_hearers = list(dict.fromkeys(hearers))
    quoted_content = '"' + truncate_chat_content(content) + '"'

    if unique_hearers:
        return "Your message " + quoted_content + " was heard by " + ", ".join(unique_hearers) + "."
    if audibility:
        return "Your message " + quoted_content + " was not heard by anyone else."
    return "Your message " + quoted_content + " was sent, but hearing feedback was unavailable."


def format_chat_notification(message, world_agent_name):
    content = str(message.get("content") or "").strip()
    if not content:
        return None
    sender = str(message.get("agent_name") or "Someone").strip() or "Someone"
    if world_agent_name and normalize_name(sender) == world_agent_name:
        return format_self_chat_feedback(message, world_agent_name)
    return "New chat message from " + sender + ": " + content


def message_key(message):
    message_id = str(message.get("id") or "").strip()
    if message_id:
        return "id:" + message_id
    return "anon:" + "|".join(
        str(message.get(key) or "").strip()
        for key in ("created_at", "agent_name", "content")
    )


def load_state(path):
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return {"baseline_loaded": False, "seen": [], "after": ""}


def save_state(path, state):
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(state, separators=(",", ":")) + "\n", encoding="utf-8")
    tmp.replace(path)


def fetch_messages(base_url, session_id, after):
    query = {"limit": str(CHAT_MESSAGES_LIMIT)}
    if after:
        query["after"] = after
    url = base_url.rstrip("/") + "/chat/messages?" + urlencode(query)
    request = Request(url, headers={"X-Session": session_id})
    with urlopen(request, timeout=2) as response:
        if response.status in (404, 405):
            return []
        if response.status != 200:
            raise RuntimeError("chat messages returned HTTP " + str(response.status))
        payload = json.loads(response.read().decode("utf-8"))
    messages = payload.get("messages")
    return messages if isinstance(messages, list) else []


def main():
    try:
        sys.stdin.read()
    except Exception:
        pass

    base_url = (
        os.environ.get("WORLD_INTERNAL_BASE_URL", "").strip()
        or os.environ.get("WORLD_BASE_URL", "").strip()
    )
    session_id = os.environ.get("WORLD_SESSION_ID", "").strip()
    world_agent_name = normalize_name(os.environ.get("WORLD_AGENT_NAME", ""))
    runtime_dir = os.environ.get("CLAWBLOX_CLAUDE_RUNTIME_DIR", "").strip()
    if not base_url or not session_id or not runtime_dir:
        return

    state_path = Path(runtime_dir) / "chat_post_tool_use_state.json"
    lock_path = Path(runtime_dir) / "chat_post_tool_use_state.lock"
    lock_path.parent.mkdir(parents=True, exist_ok=True)

    lock_handle = lock_path.open("a+", encoding="utf-8")
    try:
        try:
            import fcntl

            fcntl.flock(lock_handle.fileno(), fcntl.LOCK_EX)
        except Exception:
            pass

        state = load_state(state_path)
        seen = set(str(value) for value in state.get("seen", []) if value)
        messages = fetch_messages(base_url, session_id, str(state.get("after") or ""))

        notifications = []
        for message in reversed(messages):
            if not isinstance(message, dict):
                continue
            key = message_key(message)
            if key in seen:
                continue
            seen.add(key)

            created_at = str(message.get("created_at") or "").strip()
            if created_at and created_at > str(state.get("after") or ""):
                state["after"] = created_at

            if state.get("baseline_loaded"):
                text = format_chat_notification(message, world_agent_name)
                if text:
                    notifications.append(text)

        state["baseline_loaded"] = True
        state["seen"] = sorted(seen)[-200:]
        save_state(state_path, state)

        if notifications:
            print(json.dumps({
                "hookSpecificOutput": {
                    "hookEventName": "PostToolUse",
                    "additionalContext": "\n".join(notifications),
                }
            }))
    except (OSError, URLError, TimeoutError, RuntimeError, json.JSONDecodeError):
        return
    finally:
        try:
            lock_handle.close()
        except Exception:
            pass


if __name__ == "__main__":
    main()
"#;

const CODEX_STOP_HOOK_PY: &str = r#"#!/usr/bin/env python3
import json
import os
import sys
from pathlib import Path

payload = json.load(sys.stdin)
log_dir = os.environ.get("CLAWBLOX_CODEX_LOG_DIR", "")
session_id = str(payload.get("session_id") or "").strip()

if log_dir:
    path = Path(log_dir)
    path.mkdir(parents=True, exist_ok=True)
    raw = json.dumps(payload)
    (path / "stop.json").write_text(raw + "\n", encoding="utf-8")
    if session_id:
        (path / f"stop.{session_id}.json").write_text(raw + "\n", encoding="utf-8")

auto_loop = os.environ.get("CLAWBLOX_CODEX_AUTO_LOOP", "0")
auto_loop_message = os.environ.get("CODEX_AUTO_LOOP_MESSAGE", "")
if not auto_loop_message:
    auto_loop_message = "Please don't stop. Just continue doing whatever you want to do."

if auto_loop == "1":
    print(json.dumps({"decision": "block", "reason": auto_loop_message}))
"#;

fn require_nonempty(name: &str, value: &str) -> AgentResult<()> {
    if value.trim().is_empty() {
        return Err(AgentError::InvalidConfig(format!("{name} is required")));
    }
    Ok(())
}

fn require_nonempty_path(name: &str, value: &Path) -> AgentResult<()> {
    if value.as_os_str().is_empty() {
        return Err(AgentError::InvalidConfig(format!("{name} is required")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct EchoAgent;

    impl Agent for EchoAgent {
        fn name(&self) -> &'static str {
            "echo"
        }

        fn check_setup(&self, ctx: &AgentContext) -> AgentResult<()> {
            ctx.validate()
        }

        fn start(&self, ctx: &AgentContext) -> AgentResult<CommandSpec> {
            ctx.validate()?;
            Ok(CommandSpec::new("echo", &ctx.paths.workspace_dir)
                .arg(&ctx.settings.initial_prompt)
                .env("AGENT_MODEL", &ctx.settings.model)
                .env(
                    "SESSION_ID_FILE",
                    ctx.paths.session_id_file.display().to_string(),
                ))
        }

        fn continue_session(&self, ctx: &AgentContext, prompt: &str) -> AgentResult<CommandSpec> {
            ctx.validate()?;
            require_nonempty("prompt", prompt)?;
            Ok(CommandSpec::new("echo", &ctx.paths.workspace_dir)
                .arg(prompt)
                .env(
                    "SESSION_ID_FILE",
                    ctx.paths.session_id_file.display().to_string(),
                ))
        }

        fn configure_auto_continue(&self, ctx: &AgentContext) -> AgentResult<()> {
            ctx.settings.validate()
        }

        fn collect_transcripts(&self, ctx: &AgentContext) -> AgentResult<()> {
            ctx.paths.validate()
        }

        fn sandbox_requirements(&self, ctx: &AgentContext) -> AgentResult<SandboxRequirements> {
            ctx.validate()?;
            Ok(SandboxRequirements {
                read_only_paths: vec![ctx.paths.prompt_file.clone()],
                writable_paths: vec![
                    ctx.paths.workspace_dir.clone(),
                    ctx.paths.runtime_dir.clone(),
                    ctx.paths.transcript_dir.clone(),
                    ctx.paths.agent_home_dir.clone(),
                ],
                env_vars: vec!["HOME".to_string()],
                needs_network: ctx.settings.sandbox_enabled,
            })
        }
    }

    #[test]
    fn prepares_explicit_agent_paths_without_assuming_layout() {
        let ctx = test_context();
        ctx.prepare_paths().unwrap();

        assert!(ctx.paths.workspace_dir.is_dir());
        assert!(ctx.paths.transcript_dir.is_dir());
        assert!(ctx.paths.runtime_dir.is_dir());
        assert!(ctx.paths.agent_home_dir.is_dir());
        assert!(ctx.paths.prompt_file.parent().unwrap().is_dir());
        assert!(ctx.paths.session_id_file.parent().unwrap().is_dir());
        assert!(ctx.paths.log_file.parent().unwrap().is_dir());
    }

    #[test]
    fn agent_commands_use_provided_paths_and_settings() {
        let ctx = test_context();
        let agent = EchoAgent;

        let start = agent.start(&ctx).unwrap();
        assert_eq!(agent.name(), "echo");
        assert_eq!(start.cwd, ctx.paths.workspace_dir);
        assert_eq!(start.args, vec!["Begin".to_string()]);
        assert!(start
            .env
            .contains(&("AGENT_MODEL".to_string(), "test-model".to_string())));
        assert!(start.env.contains(&(
            "SESSION_ID_FILE".to_string(),
            ctx.paths.session_id_file.display().to_string()
        )));

        let next = agent.continue_session(&ctx, "Keep going").unwrap();
        assert_eq!(next.args, vec!["Keep going".to_string()]);
    }

    #[test]
    fn rejects_missing_required_settings() {
        let mut ctx = test_context();
        ctx.settings.auto_continue_prompt.clear();

        let err = ctx.validate().unwrap_err().to_string();
        assert!(err.contains("auto_continue_prompt is required"));
    }

    #[test]
    fn exposes_sandbox_requirements_as_data() {
        let ctx = test_context();
        let requirements = EchoAgent.sandbox_requirements(&ctx).unwrap();

        assert!(requirements.needs_network);
        assert!(requirements
            .writable_paths
            .contains(&ctx.paths.workspace_dir));
        assert!(requirements
            .read_only_paths
            .contains(&ctx.paths.prompt_file));
    }

    #[test]
    fn claude_agent_builds_start_and_continue_commands() {
        let ctx = test_context();
        ctx.prepare_paths().unwrap();
        let agent = ClaudeAgent {
            binary: PathBuf::from("/bin/echo"),
            bare: true,
            disallowed_tools: Some("WebFetch,WebSearch".to_string()),
        };

        agent.configure_auto_continue(&ctx).unwrap();
        let start = agent.start(&ctx).unwrap();

        assert_eq!(start.program, PathBuf::from("/bin/echo"));
        assert_eq!(start.cwd, ctx.paths.workspace_dir);
        assert!(start.args.contains(&"--bare".to_string()));
        assert!(start.args.contains(&"--plugin-dir".to_string()));
        assert!(start.args.contains(&"--session-id".to_string()));
        assert!(ctx.paths.session_id_file.is_file());
        assert!(claude_plugin_dir(&ctx).join("bin/stop.sh").is_file());
        assert!(claude_plugin_dir(&ctx)
            .join("bin/post_tool_use_chat.py")
            .is_file());
        for hook in [
            "session_start.sh",
            "session_end.sh",
            "stop.sh",
            "post_tool_use_chat.py",
        ] {
            let script =
                std::fs::read_to_string(claude_plugin_dir(&ctx).join("bin").join(hook)).unwrap();
            assert!(
                !script.contains("??") && !script.contains("?."),
                "{hook} should stay compatible with older Node runtimes"
            );
        }
        assert!(start.env.contains(&(
            "CLAUDE_AUTO_LOOP_MESSAGE".to_string(),
            "Keep going".to_string()
        )));
        assert!(start.env.contains(&(
            "CLAWBLOX_CLAUDE_RUNTIME_DIR".to_string(),
            ctx.paths.runtime_dir.display().to_string()
        )));

        let next = agent.continue_session(&ctx, "Later").unwrap();
        assert!(next.args.contains(&"--continue".to_string()));
        assert_eq!(next.args.last(), Some(&"Later".to_string()));
    }

    #[test]
    fn codex_agent_configures_hooks_and_resumes_saved_session() {
        let ctx = test_context();
        ctx.prepare_paths().unwrap();
        std::fs::write(&ctx.paths.prompt_file, "System prompt").unwrap();
        std::fs::write(&ctx.paths.session_id_file, "codex-session\n").unwrap();
        let agent = CodexAgent {
            binary: PathBuf::from("/bin/echo"),
        };

        agent.configure_auto_continue(&ctx).unwrap();
        let start = agent.start(&ctx).unwrap();
        assert_eq!(start.args.first(), Some(&"--no-alt-screen".to_string()));
        assert!(codex_hooks_path(&ctx).is_file());
        assert!(codex_stop_hook_path(&ctx).is_file());
        assert!(codex_home_dir(&ctx).join("config.toml").is_file());

        let next = agent.continue_session(&ctx, "Resume prompt").unwrap();
        assert_eq!(next.args.first(), Some(&"resume".to_string()));
        assert!(next.args.contains(&"codex-session".to_string()));
        assert_eq!(next.args.last(), Some(&"Resume prompt".to_string()));
    }

    #[test]
    fn claude_agent_uses_guest_paths_when_sandboxed() {
        let mut ctx = test_context();
        ctx.settings.sandbox_enabled = true;
        ctx.settings.env = vec![
            ("WORLD_BASE_URL".to_string(), "http://world".to_string()),
            (
                "CLAWBLOX_WORLD_HTTP_PROXY_PORT".to_string(),
                "43210".to_string(),
            ),
        ];
        ctx.prepare_paths().unwrap();
        std::fs::write(&ctx.paths.prompt_file, "System prompt").unwrap();
        ctx.sandbox = Some(
            SandboxLayout::for_agent(
                &ctx,
                crate::agent_sandbox::SandboxAgentKind::Claude,
                PathBuf::from("/bin/echo"),
            )
            .with_claude_plugin()
            .with_hosts_file(ctx.paths.runtime_dir.join("sandbox_hosts")),
        );
        let agent = ClaudeAgent {
            binary: PathBuf::from("/bin/echo"),
            bare: false,
            disallowed_tools: Some("WebFetch,WebSearch".to_string()),
        };

        agent.configure_auto_continue(&ctx).unwrap();
        let start = agent.start(&ctx).unwrap();

        assert_eq!(start.program, PathBuf::from("/sandbox-bin/claude"));
        assert_eq!(start.cwd, PathBuf::from("/workspace"));
        assert!(start.args.contains(&"/plugin".to_string()));
        assert!(start.args.contains(&"/tmp/system_prompt.md".to_string()));
        assert!(start
            .args
            .windows(2)
            .any(|window| window == ["--settings", "/runtime/claude_sandbox_settings.json"]));
        assert!(start
            .env
            .contains(&("CLAWBLOX_CLAUDE_LOG_DIR".to_string(), "/logs".to_string())));
        assert!(start.env.contains(&(
            "CLAWBLOX_CLAUDE_METADATA_FILE".to_string(),
            "/runtime/hook_session.env".to_string()
        )));
        assert!(start.env.contains(&(
            "CLAWBLOX_CLAUDE_RUNTIME_DIR".to_string(),
            "/runtime".to_string()
        )));
        let settings =
            std::fs::read_to_string(ctx.paths.runtime_dir.join("claude_sandbox_settings.json"))
                .unwrap();
        assert!(settings.contains("\"httpProxyPort\": 43210"));
        assert!(ctx.paths.runtime_dir.join("sandbox_hosts").is_file());
    }

    #[test]
    fn codex_agent_uses_guest_paths_and_proxy_config_when_sandboxed() {
        let mut ctx = test_context();
        ctx.settings.sandbox_enabled = true;
        ctx.settings.env = vec![
            ("WORLD_BASE_URL".to_string(), "http://world".to_string()),
            (
                "CLAWBLOX_WORLD_HTTP_PROXY_PORT".to_string(),
                "43210".to_string(),
            ),
        ];
        ctx.prepare_paths().unwrap();
        std::fs::write(&ctx.paths.prompt_file, "System prompt").unwrap();
        std::fs::write(&ctx.paths.session_id_file, "codex-session\n").unwrap();
        ctx.sandbox = Some(SandboxLayout::for_agent(
            &ctx,
            crate::agent_sandbox::SandboxAgentKind::Codex,
            PathBuf::from("/bin/echo"),
        ));
        let agent = CodexAgent {
            binary: PathBuf::from("/bin/echo"),
        };

        agent.configure_auto_continue(&ctx).unwrap();
        let start = agent.start(&ctx).unwrap();

        assert_eq!(start.program, PathBuf::from("/sandbox-bin/codex"));
        assert_eq!(start.cwd, PathBuf::from("/workspace"));
        assert!(start
            .args
            .windows(2)
            .any(|window| window == ["-C", "/workspace"]));
        assert!(start
            .args
            .windows(2)
            .any(|window| window == ["-s", "workspace-write"]));
        assert!(start
            .env
            .contains(&("HOME".to_string(), "/home/agent".to_string())));
        assert!(start.env.contains(&(
            "CLAWBLOX_CODEX_RUNTIME_DIR".to_string(),
            "/runtime".to_string()
        )));
        let config = std::fs::read_to_string(codex_home_dir(&ctx).join("config.toml")).unwrap();
        assert!(config.contains("proxy_url = \"http://127.0.0.1:43210\""));
        assert!(config.contains("[projects.\"/workspace\"]"));
        assert!(std::fs::read_to_string(codex_hooks_path(&ctx))
            .unwrap()
            .contains("/home/agent/.codex/stop_hook.py"));
    }

    #[test]
    fn codex_agent_collects_session_id_and_transcript_from_history() {
        let ctx = test_context();
        ctx.prepare_paths().unwrap();
        let sessions_dir = codex_home_dir(&ctx).join("sessions/2026/05/17");
        std::fs::create_dir_all(&sessions_dir).unwrap();
        std::fs::create_dir_all(codex_home_dir(&ctx)).unwrap();
        std::fs::write(
            codex_history_file(&ctx),
            "{\"session_id\":\"old\"}\n{\"session_id\":\"new-session\"}\n",
        )
        .unwrap();
        std::fs::write(
            sessions_dir.join("rollout-new-session.jsonl"),
            "{\"type\":\"event\"}\n",
        )
        .unwrap();

        CodexAgent::default().collect_transcripts(&ctx).unwrap();

        assert_eq!(
            std::fs::read_to_string(&ctx.paths.session_id_file).unwrap(),
            "new-session\n"
        );
        assert!(ctx.paths.transcript_dir.join("new-session.jsonl").is_file());
    }

    fn test_context() -> AgentContext {
        let root = std::env::temp_dir().join(format!(
            "clawblox-agent-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        AgentContext {
            paths: AgentPaths {
                workspace_dir: root.join("workspace"),
                prompt_file: root.join("runtime/system_prompt.md"),
                session_id_file: root.join("state/session_id.txt"),
                transcript_dir: root.join("transcripts"),
                runtime_dir: root.join("runtime"),
                log_file: root.join("logs/agent.log"),
                agent_home_dir: root.join("home"),
            },
            settings: AgentSettings {
                model: "test-model".to_string(),
                permission_mode: "test-mode".to_string(),
                initial_prompt: "Begin".to_string(),
                lifecycle_prompt: "Wrap up".to_string(),
                auto_continue_prompt: "Keep going".to_string(),
                extra_args: vec!["--flag".to_string()],
                env: vec![("WORLD_BASE_URL".to_string(), "http://world".to_string())],
                sandbox_enabled: true,
            },
            sandbox: None,
        }
    }
}
