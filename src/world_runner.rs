use std::{
    fs::OpenOptions,
    io,
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
};

#[derive(Clone, Debug)]
pub struct ExternalWorldCommand {
    pub world_dir: PathBuf,
    pub command: Vec<String>,
    pub port: u16,
    pub base_path: String,
    pub record: bool,
    pub record_dir: Option<PathBuf>,
    pub manifest_path: Option<PathBuf>,
    pub resume: Option<PathBuf>,
    pub clawblox_bin: Option<PathBuf>,
}

impl ExternalWorldCommand {
    pub fn env(&self) -> Vec<(String, String)> {
        let mut env = vec![
            ("WORLD_HOST".to_string(), "127.0.0.1".to_string()),
            ("WORLD_PORT".to_string(), self.port.to_string()),
            ("WORLD_BASE_PATH".to_string(), self.base_path.clone()),
            (
                "WORLD_RECORD".to_string(),
                if self.record { "true" } else { "false" }.to_string(),
            ),
        ];
        if let Some(record_dir) = &self.record_dir {
            env.push((
                "WORLD_RECORD_DIR".to_string(),
                record_dir.display().to_string(),
            ));
        }
        if let Some(manifest_path) = &self.manifest_path {
            env.push((
                "CLAWBLOX_MANIFEST_PATH".to_string(),
                manifest_path.display().to_string(),
            ));
        }
        if let Some(resume) = &self.resume {
            env.push(("WORLD_RESUME_PATH".to_string(), resume.display().to_string()));
        }
        if let Some(clawblox_bin) = &self.clawblox_bin {
            env.push((
                "CLAWBLOX_BIN".to_string(),
                clawblox_bin.display().to_string(),
            ));
        }
        env
    }

    pub fn executable(&self) -> Result<&str, String> {
        self.command
            .first()
            .map(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "world.toml [run].command[0] must be a non-empty executable".to_string())
    }

    pub fn args(&self) -> &[String] {
        self.command.get(1..).unwrap_or(&[])
    }
}

pub fn spawn_external_world(
    spec: &ExternalWorldCommand,
    log_file: Option<&Path>,
) -> Result<Child, String> {
    let executable = spec.executable()?;
    let mut command = Command::new(executable);
    command.args(spec.args()).current_dir(&spec.world_dir);
    for (key, value) in spec.env() {
        command.env(key, value);
    }
    if let Some(log_file) = log_file {
        if let Some(parent) = log_file.parent() {
            std::fs::create_dir_all(parent).map_err(|err| {
                format!("failed to create log directory {}: {err}", parent.display())
            })?;
        }
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file)
            .map_err(|err| format!("failed to open log file {}: {err}", log_file.display()))?;
        let err_log = log
            .try_clone()
            .map_err(|err| format!("failed to clone log file {}: {err}", log_file.display()))?;
        command.stdout(Stdio::from(log)).stderr(Stdio::from(err_log));
    }
    command.spawn().map_err(|err| {
        format!(
            "failed to start run.command executable '{}': {err}",
            executable
        )
    })
}

pub fn run_external_world_blocking(spec: &ExternalWorldCommand) -> Result<ExitStatus, String> {
    let mut child = spawn_external_world(spec, None)?;
    child
        .wait()
        .map_err(|err| format!("failed to wait for external world process: {err}"))
}

pub fn write_external_world_command_file(
    path: &Path,
    spec: &ExternalWorldCommand,
    log_file: Option<&Path>,
) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut parts = vec![
        "cd".to_string(),
        shell_quote(&spec.world_dir.display().to_string()),
        "&&".to_string(),
        "exec".to_string(),
        "env".to_string(),
    ];
    for (key, value) in spec.env() {
        parts.push(format!("{key}={}", shell_quote(&value)));
    }
    parts.push(shell_quote(spec.executable().map_err(io::Error::other)?));
    for arg in spec.args() {
        parts.push(shell_quote(arg));
    }
    if let Some(log_file) = log_file {
        parts.push(">>".to_string());
        parts.push(shell_quote(&log_file.display().to_string()));
        parts.push("2>&1".to_string());
    }
    std::fs::write(
        path,
        format!(
            "#!/usr/bin/env bash\nset -euo pipefail\n{}\n",
            parts.join(" ")
        ),
    )?;
    make_executable(path)
}

#[cfg(unix)]
fn make_executable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> io::Result<()> {
    Ok(())
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}
