use std::path::{Path, PathBuf};

use crate::agent::{AgentContext, CommandSpec};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SandboxAgentKind {
    Claude,
    Codex,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxLayout {
    pub kind: SandboxAgentKind,
    pub host_home_dir: PathBuf,
    pub host_binary: PathBuf,
    pub guest_workspace: PathBuf,
    pub guest_home: PathBuf,
    pub guest_logs: PathBuf,
    pub guest_runtime: PathBuf,
    pub guest_prompt: PathBuf,
    pub guest_plugin: Option<PathBuf>,
    pub guest_binary: PathBuf,
    pub guest_binary_root: Option<PathBuf>,
    pub host_binary_root: Option<PathBuf>,
    pub guest_binary_rel: Option<PathBuf>,
    pub host_node_root: Option<PathBuf>,
    pub guest_node_root: Option<PathBuf>,
    pub host_deps_root: Option<PathBuf>,
    pub host_hosts_file: Option<PathBuf>,
}

impl SandboxLayout {
    pub fn for_agent(ctx: &AgentContext, kind: SandboxAgentKind, host_binary: PathBuf) -> Self {
        let host_home_dir = agent_dir_from_context(ctx).join("sandbox-home");
        let guest_binary = match kind {
            SandboxAgentKind::Claude => PathBuf::from("/sandbox-bin/claude"),
            SandboxAgentKind::Codex => PathBuf::from("/sandbox-bin/codex"),
        };
        Self {
            kind,
            host_home_dir,
            host_binary,
            guest_workspace: PathBuf::from("/workspace"),
            guest_home: PathBuf::from("/home/agent"),
            guest_logs: PathBuf::from("/logs"),
            guest_runtime: PathBuf::from("/runtime"),
            guest_prompt: PathBuf::from("/tmp/system_prompt.md"),
            guest_plugin: None,
            guest_binary,
            guest_binary_root: None,
            host_binary_root: None,
            guest_binary_rel: None,
            host_node_root: None,
            guest_node_root: None,
            host_deps_root: std::env::var_os("SANDBOX_DEPS_ROOT").map(PathBuf::from),
            host_hosts_file: None,
        }
    }

    pub fn with_claude_plugin(mut self) -> Self {
        self.guest_plugin = Some(PathBuf::from("/plugin"));
        self
    }

    pub fn with_hosts_file(mut self, path: PathBuf) -> Self {
        self.host_hosts_file = Some(path);
        self
    }

    pub fn with_codex_binary_root(mut self, host_root: PathBuf, rel: PathBuf) -> Self {
        self.host_binary_root = Some(host_root);
        self.guest_binary_root = Some(PathBuf::from("/sandbox-codex"));
        self.guest_binary_rel = Some(rel);
        self
    }

    pub fn with_node_root(mut self, host_root: PathBuf) -> Self {
        self.host_node_root = Some(host_root);
        self.guest_node_root = Some(PathBuf::from("/sandbox-node"));
        self
    }

    pub fn codex_guest_program(&self) -> PathBuf {
        match (&self.guest_binary_root, &self.guest_binary_rel) {
            (Some(root), Some(rel)) => root.join(rel.strip_prefix("/").unwrap_or(rel)),
            _ => self.guest_binary.clone(),
        }
    }

    pub fn codex_guest_node(&self) -> Option<PathBuf> {
        self.guest_node_root
            .as_ref()
            .map(|root| root.join("bin/node"))
    }
}

pub fn validate_runtime(layout: &SandboxLayout) -> Result<(), String> {
    require_program("bwrap", "bubblewrap (bwrap)")?;
    if layout.kind == SandboxAgentKind::Claude {
        require_program("socat", "socat for Claude Code's native Linux sandbox")?;
    }
    if let Some(root) = &layout.host_deps_root {
        if !root.join("bin").is_dir() {
            return Err(format!(
                "SANDBOX_DEPS_ROOT must contain a bin directory: {}",
                root.display()
            ));
        }
    }
    Ok(())
}

pub fn wrap_command(
    ctx: &AgentContext,
    layout: &SandboxLayout,
    command: CommandSpec,
) -> Result<CommandSpec, String> {
    validate_runtime(layout)?;
    Ok(wrap_command_unchecked(ctx, layout, command))
}

fn wrap_command_unchecked(
    ctx: &AgentContext,
    layout: &SandboxLayout,
    command: CommandSpec,
) -> CommandSpec {
    let mut args = vec![
        "--ro-bind".to_string(),
        "/usr".to_string(),
        "/usr".to_string(),
        "--ro-bind".to_string(),
        "/bin".to_string(),
        "/bin".to_string(),
        "--ro-bind".to_string(),
        "/sbin".to_string(),
        "/sbin".to_string(),
        "--ro-bind".to_string(),
        "/lib".to_string(),
        "/lib".to_string(),
        "--ro-bind".to_string(),
        "/etc".to_string(),
        "/etc".to_string(),
        "--ro-bind".to_string(),
        "/run".to_string(),
        "/run".to_string(),
        "--proc".to_string(),
        "/proc".to_string(),
        "--dev".to_string(),
        "/dev".to_string(),
        "--tmpfs".to_string(),
        "/tmp".to_string(),
        "--dir".to_string(),
        "/sandbox-bin".to_string(),
    ];

    if layout.kind == SandboxAgentKind::Claude {
        args.extend(["--dir".to_string(), "/plugin".to_string()]);
    }
    args.extend([
        "--dir".to_string(),
        "/logs".to_string(),
        "--dir".to_string(),
        "/runtime".to_string(),
        "--dir".to_string(),
        "/home".to_string(),
        "--bind".to_string(),
        ctx.paths.workspace_dir.display().to_string(),
        layout.guest_workspace.display().to_string(),
        "--bind".to_string(),
        layout.host_home_dir.display().to_string(),
        layout.guest_home.display().to_string(),
        "--bind".to_string(),
        ctx.paths
            .log_file
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .display()
            .to_string(),
        layout.guest_logs.display().to_string(),
        "--bind".to_string(),
        ctx.paths.runtime_dir.display().to_string(),
        layout.guest_runtime.display().to_string(),
    ]);

    if layout.kind == SandboxAgentKind::Claude {
        args.extend([
            "--ro-bind".to_string(),
            ctx.paths.prompt_file.display().to_string(),
            layout.guest_prompt.display().to_string(),
        ]);
        if let Some(guest_plugin) = &layout.guest_plugin {
            args.extend([
                "--ro-bind".to_string(),
                ctx.paths
                    .runtime_dir
                    .join("claude-plugin")
                    .display()
                    .to_string(),
                guest_plugin.display().to_string(),
            ]);
        }
    }

    match (&layout.host_binary_root, &layout.guest_binary_root) {
        (Some(host_root), Some(guest_root)) => args.extend([
            "--ro-bind".to_string(),
            host_root.display().to_string(),
            guest_root.display().to_string(),
        ]),
        _ => args.extend([
            "--ro-bind".to_string(),
            layout.host_binary.display().to_string(),
            layout.guest_binary.display().to_string(),
        ]),
    }
    if let (Some(host_root), Some(guest_root)) = (&layout.host_node_root, &layout.guest_node_root) {
        args.extend([
            "--ro-bind".to_string(),
            host_root.display().to_string(),
            guest_root.display().to_string(),
        ]);
    }

    args.extend([
        "--setenv".to_string(),
        "HOME".to_string(),
        layout.guest_home.display().to_string(),
        "--setenv".to_string(),
        "USER".to_string(),
        "agent".to_string(),
        "--setenv".to_string(),
        "LOGNAME".to_string(),
        "agent".to_string(),
        "--setenv".to_string(),
        "SHELL".to_string(),
        "/bin/bash".to_string(),
        "--setenv".to_string(),
        "PATH".to_string(),
        sandbox_path(ctx, layout),
        "--chdir".to_string(),
        layout.guest_workspace.display().to_string(),
        "--die-with-parent".to_string(),
    ]);
    if ctx.paths.workspace_dir.join(".venv").is_dir() {
        args.extend([
            "--setenv".to_string(),
            "VIRTUAL_ENV".to_string(),
            layout.guest_workspace.join(".venv").display().to_string(),
        ]);
    }
    if layout.kind == SandboxAgentKind::Claude {
        for name in [
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_AUTH_TOKEN",
            "CLAUDE_CONFIG_DIR",
            "CLAUDE_CODE_EXECPATH",
            "CLAUDE_CODE_ENTRYPOINT",
            "CLAUDECODE",
        ] {
            args.extend(["--unsetenv".to_string(), name.to_string()]);
        }
    }

    if let Some(root) = &layout.host_deps_root {
        args.extend([
            "--ro-bind".to_string(),
            root.display().to_string(),
            "/sandbox-deps".to_string(),
        ]);
        if root.join("lib").is_dir() {
            args.extend([
                "--setenv".to_string(),
                "LD_LIBRARY_PATH".to_string(),
                "/sandbox-deps/lib".to_string(),
            ]);
        }
    }
    if Path::new("/lib64").is_dir() {
        args.extend([
            "--ro-bind".to_string(),
            "/lib64".to_string(),
            "/lib64".to_string(),
        ]);
    }
    if let Some(hosts_file) = &layout.host_hosts_file {
        if hosts_file.is_file() {
            args.extend([
                "--ro-bind".to_string(),
                hosts_file.display().to_string(),
                "/etc/hosts".to_string(),
            ]);
        }
    }
    if Path::new("/mnt/wsl/resolv.conf").is_file() {
        args.extend([
            "--ro-bind".to_string(),
            "/mnt/wsl/resolv.conf".to_string(),
            "/mnt/wsl/resolv.conf".to_string(),
        ]);
    }

    args.push("--".to_string());
    args.push(command.program.display().to_string());
    args.extend(command.args);

    CommandSpec {
        program: PathBuf::from("bwrap"),
        args,
        env: command.env,
        cwd: ctx.paths.workspace_dir.clone(),
    }
}

fn sandbox_path(ctx: &AgentContext, layout: &SandboxLayout) -> String {
    let mut entries = Vec::new();
    if ctx.paths.workspace_dir.join(".venv/bin").is_dir() {
        entries.push("/workspace/.venv/bin");
    }
    if layout.host_deps_root.is_some() {
        entries.extend([
            "/home/agent/.local/bin",
            "/sandbox-bin",
            "/sandbox-deps/bin",
            "/usr/bin",
            "/bin",
        ]);
    } else {
        entries.extend(["/home/agent/.local/bin", "/sandbox-bin", "/usr/bin", "/bin"]);
    }
    entries.join(":")
}

fn require_program(name: &str, label: &str) -> Result<(), String> {
    let Some(paths) = std::env::var_os("PATH") else {
        return Err(format!("{label} was not found in PATH"));
    };
    for dir in std::env::split_paths(&paths) {
        if dir.join(name).exists() {
            return Ok(());
        }
    }
    Err(format!("{label} was not found in PATH"))
}

fn agent_dir_from_context(ctx: &AgentContext) -> PathBuf {
    ctx.paths
        .runtime_dir
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{AgentPaths, AgentSettings};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn wraps_claude_command_with_expected_mounts_and_guest_paths() {
        let root = test_root();
        let ctx = test_context(root.clone());
        let layout = SandboxLayout::for_agent(
            &ctx,
            SandboxAgentKind::Claude,
            PathBuf::from("/usr/local/bin/claude"),
        )
        .with_claude_plugin()
        .with_hosts_file(ctx.paths.runtime_dir.join("sandbox_hosts"));
        let command = CommandSpec::new("/sandbox-bin/claude", "/workspace")
            .arg("--plugin-dir")
            .arg("/plugin")
            .arg("Begin")
            .env("WORLD_BASE_URL", "http://world");

        let wrapped = wrap_command_unchecked(&ctx, &layout, command);

        assert_eq!(wrapped.program, PathBuf::from("bwrap"));
        assert!(has_arg_triple(
            &wrapped.args,
            "--bind",
            &ctx.paths.workspace_dir.display().to_string(),
            "/workspace"
        ));
        assert!(has_arg_triple(
            &wrapped.args,
            "--bind",
            &root.join("sandbox-home").display().to_string(),
            "/home/agent"
        ));
        assert!(has_arg_triple(
            &wrapped.args,
            "--ro-bind",
            &ctx.paths.prompt_file.display().to_string(),
            "/tmp/system_prompt.md"
        ));
        assert!(has_arg_triple(
            &wrapped.args,
            "--ro-bind",
            &ctx.paths
                .runtime_dir
                .join("claude-plugin")
                .display()
                .to_string(),
            "/plugin"
        ));
        assert!(wrapped
            .args
            .windows(2)
            .any(|window| window == ["--", "/sandbox-bin/claude"]));
        assert!(has_arg_pair(
            &wrapped.args,
            "--unsetenv",
            "CLAUDE_CONFIG_DIR"
        ));
        assert!(has_arg_pair(
            &wrapped.args,
            "--unsetenv",
            "ANTHROPIC_API_KEY"
        ));
    }

    #[test]
    fn wraps_codex_package_root_when_binary_is_node_package() {
        let root = test_root();
        let node_root = root.join("node");
        let ctx = test_context(root.clone());
        let layout = SandboxLayout::for_agent(
            &ctx,
            SandboxAgentKind::Codex,
            root.join("node_modules/.bin/codex"),
        )
        .with_codex_binary_root(root.clone(), PathBuf::from("/node_modules/.bin/codex"))
        .with_node_root(node_root.clone());
        let command = CommandSpec::new(layout.codex_guest_node().unwrap(), "/workspace")
            .arg(layout.codex_guest_program().display().to_string())
            .arg("--no-alt-screen");

        let wrapped = wrap_command_unchecked(&ctx, &layout, command);

        assert!(has_arg_triple(
            &wrapped.args,
            "--ro-bind",
            &root.display().to_string(),
            "/sandbox-codex"
        ));
        assert!(has_arg_triple(
            &wrapped.args,
            "--ro-bind",
            &node_root.display().to_string(),
            "/sandbox-node"
        ));
        assert!(wrapped
            .args
            .windows(2)
            .any(|window| window == ["--", "/sandbox-node/bin/node"]));
        assert!(wrapped.args.windows(2).any(|window| window
            == [
                "/sandbox-node/bin/node",
                "/sandbox-codex/node_modules/.bin/codex"
            ]));
    }

    #[test]
    fn exposes_workspace_venv_inside_sandbox() {
        let root = test_root();
        let ctx = test_context(root.clone());
        std::fs::create_dir_all(ctx.paths.workspace_dir.join(".venv/bin")).unwrap();
        let layout = SandboxLayout::for_agent(
            &ctx,
            SandboxAgentKind::Codex,
            root.join("node_modules/.bin/codex"),
        );
        let command = CommandSpec::new("/sandbox-bin/codex", "/workspace");

        let wrapped = wrap_command_unchecked(&ctx, &layout, command);

        assert!(has_arg_pair(&wrapped.args, "--setenv", "PATH"));
        assert!(wrapped.args.windows(3).any(|window| window
            == [
                "--setenv",
                "PATH",
                "/workspace/.venv/bin:/home/agent/.local/bin:/sandbox-bin:/usr/bin:/bin"
            ]));
        assert!(wrapped
            .args
            .windows(3)
            .any(|window| window == ["--setenv", "VIRTUAL_ENV", "/workspace/.venv"]));
    }

    fn has_arg_triple(args: &[String], flag: &str, host: &str, guest: &str) -> bool {
        args.windows(3).any(|window| window == [flag, host, guest])
    }

    fn has_arg_pair(args: &[String], flag: &str, value: &str) -> bool {
        args.windows(2).any(|window| window == [flag, value])
    }

    fn test_context(root: PathBuf) -> AgentContext {
        AgentContext {
            paths: AgentPaths {
                workspace_dir: root.join("workspace"),
                prompt_file: root.join("runtime/system_prompt.md"),
                session_id_file: root.join("state/session_id.txt"),
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
                sandbox_enabled: true,
            },
            sandbox: None,
        }
    }

    fn test_root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "clawblox-agent-sandbox-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
}
