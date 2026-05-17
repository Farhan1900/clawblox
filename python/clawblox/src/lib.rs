use std::{
    fs::OpenOptions,
    io::{Read, Write},
    net::TcpStream,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use clawblox::agent_runner::{
    send_agent, start_agent, stop_agent, AgentKind, AgentSendOptions, AgentStartOptions,
    AgentStopOptions,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

#[pyclass(name = "Agent")]
#[derive(Clone, Debug)]
struct PyAgent {
    agent: String,
    name: String,
    binary: Option<PathBuf>,
    dir: PathBuf,
    model: String,
    permission_mode: String,
    auto_continue_prompt: String,
    tmux: Option<String>,
    bare: bool,
    extra_args: Vec<String>,
    sandbox: bool,
}

#[pyclass(name = "AgentStartResult")]
#[derive(Clone, Debug)]
struct PyAgentStartResult {
    tmux_session: String,
    agent_dir: PathBuf,
    command_file: PathBuf,
    events_file: PathBuf,
    attach_command: String,
}

#[pyclass(name = "AgentStopResult")]
#[derive(Clone, Debug)]
struct PyAgentStopResult {
    tmux_session: Option<String>,
    agent_dir: PathBuf,
    events_file: PathBuf,
    log_file: PathBuf,
}

#[pyclass(name = "World")]
#[derive(Debug)]
struct PyWorld {
    dir: PathBuf,
    config: serde_json::Value,
    port: Option<u16>,
    base_path: String,
    record: bool,
    record_dir: Option<PathBuf>,
    health_timeout: u64,
    port_wait: u64,
    command_file: Option<PathBuf>,
    log_file: Option<PathBuf>,
    process: Option<Child>,
}

#[pyclass(name = "WorldStartResult")]
#[derive(Clone, Debug)]
struct PyWorldStartResult {
    command: Vec<String>,
    start_command: Vec<String>,
    delegated: bool,
    run_command: Option<Vec<String>>,
    env: Vec<(String, String)>,
    url: String,
    command_file: PathBuf,
    log_file: PathBuf,
}

#[pymethods]
impl PyAgent {
    #[new]
    #[pyo3(signature = (*, agent, name, dir, model, binary = None, permission_mode = "bypassPermissions".to_string(), auto_continue_prompt = "Please don't stop. Just continue doing whatever you want to do.".to_string(), tmux = None, bare = false, extra_args = Vec::new(), sandbox = false))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        agent: String,
        name: String,
        dir: PathBuf,
        model: String,
        binary: Option<PathBuf>,
        permission_mode: String,
        auto_continue_prompt: String,
        tmux: Option<String>,
        bare: bool,
        extra_args: Vec<String>,
        sandbox: bool,
    ) -> Self {
        Self {
            agent,
            name,
            binary,
            dir,
            model,
            permission_mode,
            auto_continue_prompt,
            tmux,
            bare,
            extra_args,
            sandbox,
        }
    }

    #[getter]
    fn agent(&self) -> &str {
        &self.agent
    }

    #[getter]
    fn name(&self) -> &str {
        &self.name
    }

    #[getter]
    fn dir(&self) -> PathBuf {
        self.dir.clone()
    }

    #[getter]
    fn workspace_dir(&self) -> PathBuf {
        self.dir.join("workspace")
    }

    #[getter]
    fn runtime_dir(&self) -> PathBuf {
        self.dir.join("runtime")
    }

    #[getter]
    fn log_dir(&self) -> PathBuf {
        self.dir.join("logs")
    }

    #[getter]
    fn events_file(&self) -> PathBuf {
        self.dir.join("events.jsonl")
    }

    #[getter]
    fn visible_workspace_dir(&self) -> String {
        if self.sandbox {
            "/workspace".to_string()
        } else {
            self.workspace_dir().display().to_string()
        }
    }

    #[pyo3(signature = (*, initial_prompt, system_prompt, env = None))]
    fn start(
        &self,
        initial_prompt: String,
        system_prompt: String,
        env: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyAgentStartResult> {
        let env = dict_to_env(env)?;
        let kind = parse_agent_kind(&self.agent)?;
        let started = start_agent(AgentStartOptions {
            kind,
            name: self.name.clone(),
            binary: self.binary.clone(),
            agent_dir: self.dir.clone(),
            model: self.model.clone(),
            permission_mode: self.permission_mode.clone(),
            initial_prompt,
            system_prompt,
            auto_continue_prompt: self.auto_continue_prompt.clone(),
            tmux_session: self.tmux.clone(),
            env,
            extra_args: self.extra_args.clone(),
            sandbox: self.sandbox,
            bare: self.bare,
        })
        .map_err(PyAgentError::new_err)?;

        Ok(PyAgentStartResult {
            tmux_session: started.tmux_session,
            agent_dir: started.agent_dir,
            command_file: started.command_file,
            events_file: started.events_file,
            attach_command: started.attach_command,
        })
    }

    #[pyo3(signature = (*, prompt, env = None))]
    fn send(
        &self,
        prompt: String,
        env: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyAgentStartResult> {
        let env = dict_to_env(env)?;
        let kind = parse_agent_kind(&self.agent)?;
        let started = send_agent(AgentSendOptions {
            kind,
            name: self.name.clone(),
            binary: self.binary.clone(),
            agent_dir: self.dir.clone(),
            model: self.model.clone(),
            permission_mode: self.permission_mode.clone(),
            prompt,
            auto_continue_prompt: self.auto_continue_prompt.clone(),
            tmux_session: self.tmux.clone(),
            env,
            extra_args: self.extra_args.clone(),
            sandbox: self.sandbox,
            bare: self.bare,
        })
        .map_err(PyAgentError::new_err)?;

        Ok(PyAgentStartResult {
            tmux_session: started.tmux_session,
            agent_dir: started.agent_dir,
            command_file: started.command_file,
            events_file: started.events_file,
            attach_command: started.attach_command,
        })
    }

    #[pyo3(signature = (*, grace_seconds = 10, env = None))]
    fn stop(
        &self,
        grace_seconds: u64,
        env: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyAgentStopResult> {
        let env = dict_to_env(env)?;
        let kind = parse_agent_kind(&self.agent)?;
        let stopped = stop_agent(AgentStopOptions {
            kind,
            name: self.name.clone(),
            binary: self.binary.clone(),
            agent_dir: self.dir.clone(),
            model: self.model.clone(),
            permission_mode: self.permission_mode.clone(),
            auto_continue_prompt: self.auto_continue_prompt.clone(),
            tmux_session: self.tmux.clone(),
            env,
            extra_args: self.extra_args.clone(),
            sandbox: self.sandbox,
            bare: self.bare,
            grace_seconds,
        })
        .map_err(PyAgentError::new_err)?;

        Ok(PyAgentStopResult {
            tmux_session: stopped.tmux_session,
            agent_dir: stopped.agent_dir,
            events_file: stopped.events_file,
            log_file: stopped.log_file,
        })
    }
}

#[pymethods]
impl PyWorld {
    #[new]
    #[pyo3(signature = (*, dir))]
    fn new(dir: PathBuf) -> PyResult<Self> {
        let world_toml = dir.join("world.toml");
        if !world_toml.is_file() {
            return Err(PyAgentError::new_err(format!(
                "world directory must contain world.toml: {}",
                dir.display()
            )));
        }
        let config = read_world_config_value(&world_toml)?;
        Ok(Self {
            dir,
            config,
            port: None,
            base_path: "/".to_string(),
            record: false,
            record_dir: None,
            health_timeout: 30,
            port_wait: 120,
            command_file: None,
            log_file: None,
            process: None,
        })
    }

    #[getter]
    fn dir(&self) -> PathBuf {
        self.dir.clone()
    }

    #[getter]
    fn config<'py>(&self, py: Python<'py>) -> PyResult<PyObject> {
        json_to_py(py, &self.config)
    }

    #[getter]
    fn name(&self) -> Option<String> {
        self.config
            .get("name")
            .and_then(|value| value.as_str())
            .map(ToString::to_string)
    }

    #[getter]
    fn description(&self) -> Option<String> {
        self.config
            .get("description")
            .and_then(|value| value.as_str())
            .map(ToString::to_string)
    }

    #[getter]
    fn api_doc_path(&self) -> String {
        self.api_doc_path_value()
    }

    #[getter]
    fn api<'py>(&self, py: Python<'py>) -> PyResult<PyObject> {
        let api = serde_json::json!({
            "doc_path": self.api_doc_path_value(),
        });
        json_to_py(py, &api)
    }

    #[getter]
    fn scripts<'py>(&self, py: Python<'py>) -> PyResult<PyObject> {
        self.config_section(py, "scripts")
    }

    #[getter]
    fn renderer<'py>(&self, py: Python<'py>) -> PyResult<PyObject> {
        self.config_section(py, "renderer")
    }

    #[getter]
    fn runtime<'py>(&self, py: Python<'py>) -> PyResult<PyObject> {
        self.config_section(py, "runtime")
    }

    #[getter]
    fn run<'py>(&self, py: Python<'py>) -> PyResult<PyObject> {
        self.config_section(py, "run")
    }

    #[getter]
    fn run_command(&self) -> Option<Vec<String>> {
        self.config
            .get("run")
            .and_then(|run| run.get("command"))
            .and_then(|command| command.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(ToString::to_string))
                    .collect()
            })
    }

    #[getter]
    fn uses_run_command(&self) -> bool {
        self.run_command().is_some()
    }

    #[getter]
    fn runtime_mode(&self) -> String {
        self.config
            .get("runtime")
            .and_then(|runtime| runtime.get("mode"))
            .and_then(|value| value.as_str())
            .unwrap_or("clawblox")
            .to_string()
    }

    #[getter]
    fn port(&self) -> Option<u16> {
        self.port
    }

    #[getter]
    fn url(&self) -> PyResult<String> {
        let port = self.running_port()?;
        Ok(format!("http://localhost:{}{}", port, self.base_path)
            .trim_end_matches('/')
            .to_string())
    }

    #[getter]
    fn command_file(&self) -> Option<PathBuf> {
        self.command_file.clone()
    }

    #[getter]
    fn log_file(&self) -> Option<PathBuf> {
        self.log_file.clone()
    }

    #[getter]
    fn start_command(&self) -> Vec<String> {
        self.run_command().unwrap_or_else(|| {
            vec![
                "clawblox".to_string(),
                "run".to_string(),
                "{world_dir}".to_string(),
                "--port".to_string(),
                "{port}".to_string(),
                "--base-path".to_string(),
                "{base_path}".to_string(),
            ]
        })
    }

    #[getter]
    fn start_env<'py>(&self, py: Python<'py>) -> PyResult<PyObject> {
        let env = if self.uses_run_command() {
            serde_json::json!({
                "CLAWBLOX_PORT": "{port}",
                "CLAWBLOX_BIN": "{clawblox_bin}",
            })
        } else {
            serde_json::json!({})
        };
        json_to_py(py, &env)
    }

    #[pyo3(signature = (*, port, base_path = "/".to_string(), record = false, record_dir = None, health_timeout = 30, port_wait = 120, command_file = None, log_file = None))]
    #[allow(clippy::too_many_arguments)]
    fn start(
        &mut self,
        port: u16,
        base_path: String,
        record: bool,
        record_dir: Option<PathBuf>,
        health_timeout: u64,
        port_wait: u64,
        command_file: Option<PathBuf>,
        log_file: Option<PathBuf>,
    ) -> PyResult<PyWorldStartResult> {
        if let Some(process) = self.process.as_mut() {
            if process.try_wait().map_err(io_error)?.is_none() {
                return Ok(PyWorldStartResult {
                    command: build_world_start_argv(
                        &resolve_clawblox_bin()?,
                        &self.dir,
                        self.running_port()?,
                        &self.base_path,
                        self.record,
                        self.record_dir.as_ref(),
                    ),
                    start_command: self.start_command(),
                    delegated: self.uses_run_command(),
                    run_command: self.run_command(),
                    env: self.resolved_start_env(self.running_port()?, &resolve_clawblox_bin()?),
                    url: self.url()?,
                    command_file: self.running_command_file()?,
                    log_file: self.running_log_file()?,
                });
            }
        }
        let state_root = record_dir
            .as_ref()
            .and_then(|path| path.parent().map(|path| path.to_path_buf()))
            .unwrap_or_else(|| self.dir.join(".clawblox"));
        self.port = Some(port);
        self.base_path = normalize_base_path(&base_path);
        if self.uses_run_command() && self.base_path != "/" {
            return Err(PyAgentError::new_err(
                "world.toml [run].command is configured, so base_path must be '/'".to_string(),
            ));
        }
        if self.uses_run_command() && record {
            return Err(PyAgentError::new_err(
                "world.toml [run].command is configured, so Clawblox recording is not available for this world start".to_string(),
            ));
        }
        self.record = record;
        self.record_dir = record_dir;
        self.health_timeout = health_timeout;
        self.port_wait = port_wait;
        self.command_file =
            Some(command_file.unwrap_or_else(|| state_root.join("runtime/world.sh")));
        self.log_file = Some(log_file.unwrap_or_else(|| state_root.join("logs/world.log")));

        wait_for_port_release(port, Duration::from_secs(self.port_wait))?;

        let bin = resolve_clawblox_bin()?;
        let command = build_world_start_argv(
            &bin,
            &self.dir,
            port,
            &self.base_path,
            self.record,
            self.record_dir.as_ref(),
        );
        let command_file = self.running_command_file()?;
        let log_file = self.running_log_file()?;
        write_world_command_file(&command_file, &bin, self)?;
        if let Some(parent) = log_file.parent() {
            std::fs::create_dir_all(parent).map_err(io_error)?;
        }
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
            .map_err(io_error)?;
        let err_log = log.try_clone().map_err(io_error)?;
        let child = Command::new(&bin)
            .arg("run")
            .arg(&self.dir)
            .arg("--port")
            .arg(port.to_string())
            .arg("--base-path")
            .arg(&self.base_path)
            .args(if self.record {
                vec!["--record"]
            } else {
                vec![]
            })
            .args(
                self.record_dir
                    .as_ref()
                    .map(|path| vec!["--record-dir".to_string(), path.display().to_string()])
                    .unwrap_or_default(),
            )
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(err_log))
            .spawn()
            .map_err(|err| {
                PyAgentError::new_err(format!(
                    "failed to start world command {}: {err}\ncommand file: {}\nlog file: {}",
                    bin.display(),
                    command_file.display(),
                    log_file.display()
                ))
            })?;
        self.process = Some(child);

        let health_path = join_url_path(&self.base_path, "/api.md");
        let deadline = Instant::now() + Duration::from_secs(self.health_timeout);
        while Instant::now() < deadline {
            if let Some(process) = self.process.as_mut() {
                if let Some(status) = process.try_wait().map_err(io_error)? {
                    return Err(PyAgentError::new_err(format!(
                        "world process exited before becoming healthy with status {status}\ncommand file: {}\nlog file: {}",
                        command_file.display(),
                        log_file.display()
                    )));
                }
            }
            if http_request("GET", port, &health_path, &[], "")
                .is_ok_and(|response| response.status == 200)
            {
                return Ok(PyWorldStartResult {
                    command,
                    start_command: self.start_command(),
                    delegated: self.uses_run_command(),
                    run_command: self.run_command(),
                    env: self.resolved_start_env(port, &bin),
                    url: self.url()?,
                    command_file,
                    log_file,
                });
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        Err(PyAgentError::new_err(format!(
            "world did not become healthy at {} within {}s\ncommand file: {}\nlog file: {}",
            self.url()?,
            self.health_timeout,
            command_file.display(),
            log_file.display()
        )))
    }

    #[pyo3(signature = (*, agent))]
    fn connect<'py>(&self, py: Python<'py>, agent: &PyAgent) -> PyResult<Bound<'py, PyDict>> {
        let port = self.running_port()?;
        let path = format!(
            "{}?name={}",
            join_url_path(&self.base_path, "/join"),
            percent_encode(agent.name())
        );
        let response = http_request("POST", port, &path, &[], "")?;
        if response.status != 200 {
            return Err(PyAgentError::new_err(format!(
                "failed to join world at {}{}: HTTP {}\nbody: {}",
                self.url()?,
                path,
                response.status,
                response.body
            )));
        }
        let value: serde_json::Value = serde_json::from_str(&response.body).map_err(|err| {
            PyAgentError::new_err(format!("failed to parse world join response: {err}"))
        })?;
        let session = value
            .get("session")
            .and_then(|value| value.as_str())
            .ok_or_else(|| {
                PyAgentError::new_err("world join response missing session".to_string())
            })?;
        let agent_id = value
            .get("agent_id")
            .and_then(|value| value.as_str())
            .unwrap_or("");

        let session_file = agent.dir.join("world_session.txt");
        if let Some(parent) = session_file.parent() {
            std::fs::create_dir_all(parent).map_err(io_error)?;
        }
        std::fs::write(&session_file, format!("{session}\n")).map_err(io_error)?;

        let headers = PyDict::new_bound(py);
        headers.set_item("X-Session", session)?;
        let access = PyDict::new_bound(py);
        access.set_item("headers", headers)?;
        access.set_item("session", session)?;
        access.set_item("agent_id", agent_id)?;
        Ok(access)
    }

    #[pyo3(signature = (*, path = "/api.md".to_string(), access = None))]
    fn api_path(&self, path: String, access: Option<&Bound<'_, PyDict>>) -> PyResult<String> {
        let port = self.running_port()?;
        let headers = access_headers(access)?;
        let request_path = join_url_path(&self.base_path, &path);
        match http_request("GET", port, &request_path, &headers, "") {
            Ok(response) if response.status == 200 => Ok(path),
            Ok(response) => Err(PyAgentError::new_err(format!(
                "world API path {} returned HTTP {}\nbody: {}",
                request_path, response.status, response.body
            ))),
            Err(err) => Err(err),
        }
    }

    #[pyo3(signature = (*, grace_seconds = 10))]
    fn stop(&mut self, grace_seconds: u64) -> PyResult<()> {
        let Some(mut process) = self.process.take() else {
            return Ok(());
        };
        terminate_process(process.id());
        let deadline = Instant::now() + Duration::from_secs(grace_seconds);
        while Instant::now() < deadline {
            if process.try_wait().map_err(io_error)?.is_some() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        process.kill().map_err(io_error)?;
        let _ = process.wait();
        Ok(())
    }
}

impl PyWorld {
    fn api_doc_path_value(&self) -> String {
        self.config
            .get("api")
            .and_then(|api| api.get("doc"))
            .and_then(|value| value.as_str())
            .or_else(|| {
                self.config
                    .get("scripts")
                    .and_then(|scripts| scripts.get("skill"))
                    .and_then(|value| value.as_str())
            })
            .unwrap_or("API.md")
            .to_string()
    }

    fn config_section(&self, py: Python<'_>, name: &str) -> PyResult<PyObject> {
        json_to_py(
            py,
            self.config
                .get(name)
                .unwrap_or(&serde_json::Value::Object(Default::default())),
        )
    }

    fn resolved_start_env(&self, port: u16, bin: &PathBuf) -> Vec<(String, String)> {
        if self.uses_run_command() {
            vec![
                ("CLAWBLOX_PORT".to_string(), port.to_string()),
                ("CLAWBLOX_BIN".to_string(), bin.display().to_string()),
            ]
        } else {
            Vec::new()
        }
    }

    fn running_port(&self) -> PyResult<u16> {
        self.port.ok_or_else(|| {
            PyAgentError::new_err(
                "world has not been started; call world.start(port=...) first".to_string(),
            )
        })
    }

    fn running_command_file(&self) -> PyResult<PathBuf> {
        self.command_file.clone().ok_or_else(|| {
            PyAgentError::new_err(
                "world has not been started; no command file exists yet".to_string(),
            )
        })
    }

    fn running_log_file(&self) -> PyResult<PathBuf> {
        self.log_file.clone().ok_or_else(|| {
            PyAgentError::new_err("world has not been started; no log file exists yet".to_string())
        })
    }
}

impl Drop for PyWorld {
    fn drop(&mut self) {
        let _ = self.stop(2);
    }
}

#[pymethods]
impl PyWorldStartResult {
    #[getter]
    fn command(&self) -> Vec<String> {
        self.command.clone()
    }

    #[getter]
    fn start_command(&self) -> Vec<String> {
        self.start_command.clone()
    }

    #[getter]
    fn delegated(&self) -> bool {
        self.delegated
    }

    #[getter]
    fn run_command(&self) -> Option<Vec<String>> {
        self.run_command.clone()
    }

    #[getter]
    fn env<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new_bound(py);
        for (key, value) in &self.env {
            dict.set_item(key, value)?;
        }
        Ok(dict)
    }

    #[getter]
    fn url(&self) -> &str {
        &self.url
    }

    #[getter]
    fn command_file(&self) -> PathBuf {
        self.command_file.clone()
    }

    #[getter]
    fn log_file(&self) -> PathBuf {
        self.log_file.clone()
    }
}

#[pymethods]
impl PyAgentStartResult {
    #[getter]
    fn tmux_session(&self) -> &str {
        &self.tmux_session
    }

    #[getter]
    fn agent_dir(&self) -> PathBuf {
        self.agent_dir.clone()
    }

    #[getter]
    fn command_file(&self) -> PathBuf {
        self.command_file.clone()
    }

    #[getter]
    fn events_file(&self) -> PathBuf {
        self.events_file.clone()
    }

    #[getter]
    fn attach_command(&self) -> &str {
        &self.attach_command
    }
}

#[pymethods]
impl PyAgentStopResult {
    #[getter]
    fn tmux_session(&self) -> Option<&str> {
        self.tmux_session.as_deref()
    }

    #[getter]
    fn agent_dir(&self) -> PathBuf {
        self.agent_dir.clone()
    }

    #[getter]
    fn events_file(&self) -> PathBuf {
        self.events_file.clone()
    }

    #[getter]
    fn log_file(&self) -> PathBuf {
        self.log_file.clone()
    }
}

#[derive(Debug)]
struct PyAgentError;

impl PyAgentError {
    fn new_err(message: String) -> PyErr {
        pyo3::exceptions::PyRuntimeError::new_err(message)
    }
}

fn parse_agent_kind(value: &str) -> PyResult<AgentKind> {
    match value {
        "claude" => Ok(AgentKind::Claude),
        "codex" => Ok(AgentKind::Codex),
        other => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "unsupported agent '{other}' (expected 'claude' or 'codex')"
        ))),
    }
}

fn dict_to_env(env: Option<&Bound<'_, PyDict>>) -> PyResult<Vec<(String, String)>> {
    let Some(env) = env else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for (key, value) in env.iter() {
        out.push((key.extract()?, value.extract()?));
    }
    Ok(out)
}

#[derive(Debug)]
struct HttpResponse {
    status: u16,
    body: String,
}

fn resolve_clawblox_bin() -> PyResult<PathBuf> {
    if let Ok(value) = std::env::var("CLAWBLOX_BIN") {
        let path = PathBuf::from(value);
        if path.exists() {
            return Ok(path);
        }
    }
    for candidate in [
        "target/debug/clawblox",
        "target/release/clawblox",
        "clawblox",
    ] {
        let path = PathBuf::from(candidate);
        if candidate.contains('/') {
            if path.exists() {
                return Ok(path);
            }
        } else if command_exists(candidate) {
            return Ok(path);
        }
    }
    Err(PyAgentError::new_err(
        "could not find clawblox binary; set CLAWBLOX_BIN or build target/debug/clawblox"
            .to_string(),
    ))
}

fn read_world_config_value(path: &PathBuf) -> PyResult<serde_json::Value> {
    clawblox::config::WorldConfig::from_file(path)
        .map_err(|err| PyAgentError::new_err(err.to_string()))?;
    let content = std::fs::read_to_string(path).map_err(io_error)?;
    let value: toml::Value = toml::from_str(&content).map_err(|err| {
        PyAgentError::new_err(format!("failed to parse {}: {err}", path.display()))
    })?;
    serde_json::to_value(value).map_err(|err| {
        PyAgentError::new_err(format!(
            "failed to convert {} to JSON: {err}",
            path.display()
        ))
    })
}

fn json_to_py(py: Python<'_>, value: &serde_json::Value) -> PyResult<PyObject> {
    match value {
        serde_json::Value::Null => Ok(py.None()),
        serde_json::Value::Bool(value) => Ok(value.into_py(py)),
        serde_json::Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                Ok(value.into_py(py))
            } else if let Some(value) = value.as_u64() {
                Ok(value.into_py(py))
            } else if let Some(value) = value.as_f64() {
                Ok(value.into_py(py))
            } else {
                Ok(py.None())
            }
        }
        serde_json::Value::String(value) => Ok(value.into_py(py)),
        serde_json::Value::Array(items) => {
            let list = PyList::empty_bound(py);
            for item in items {
                list.append(json_to_py(py, item)?)?;
            }
            Ok(list.into_py(py))
        }
        serde_json::Value::Object(items) => {
            let dict = PyDict::new_bound(py);
            for (key, value) in items {
                dict.set_item(key, json_to_py(py, value)?)?;
            }
            Ok(dict.into_py(py))
        }
    }
}

fn command_exists(program: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).exists()))
        .unwrap_or(false)
}

fn build_world_start_argv(
    bin: &PathBuf,
    dir: &PathBuf,
    port: u16,
    base_path: &str,
    record: bool,
    record_dir: Option<&PathBuf>,
) -> Vec<String> {
    let mut argv = vec![
        bin.display().to_string(),
        "run".to_string(),
        dir.display().to_string(),
        "--port".to_string(),
        port.to_string(),
        "--base-path".to_string(),
        base_path.to_string(),
    ];
    if record {
        argv.push("--record".to_string());
    }
    if let Some(record_dir) = record_dir {
        argv.push("--record-dir".to_string());
        argv.push(record_dir.display().to_string());
    }
    argv
}

fn write_world_command_file(path: &PathBuf, bin: &PathBuf, world: &PyWorld) -> PyResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(io_error)?;
    }
    let mut parts = vec![
        shell_quote(&bin.display().to_string()),
        "run".to_string(),
        shell_quote(&world.dir.display().to_string()),
        "--port".to_string(),
        world.running_port()?.to_string(),
        "--base-path".to_string(),
        shell_quote(&world.base_path),
    ];
    if world.record {
        parts.push("--record".to_string());
    }
    if let Some(record_dir) = &world.record_dir {
        parts.push("--record-dir".to_string());
        parts.push(shell_quote(&record_dir.display().to_string()));
    }
    parts.push(">>".to_string());
    parts.push(shell_quote(
        &world.running_log_file()?.display().to_string(),
    ));
    parts.push("2>&1".to_string());
    std::fs::write(
        path,
        format!(
            "#!/usr/bin/env bash\nset -euo pipefail\n{}\n",
            parts.join(" ")
        ),
    )
    .map_err(io_error)
}

fn wait_for_port_release(port: u16, timeout: Duration) -> PyResult<()> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if TcpStream::connect(("127.0.0.1", port)).is_err() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Err(PyAgentError::new_err(format!(
        "port {port} is still in use after {}s",
        timeout.as_secs()
    )))
}

fn http_request(
    method: &str,
    port: u16,
    path: &str,
    headers: &[(String, String)],
    body: &str,
) -> PyResult<HttpResponse> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).map_err(io_error)?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(io_error)?;
    let mut request = format!(
        "{method} {path} HTTP/1.1\r\nHost: localhost:{port}\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    for (name, value) in headers {
        request.push_str(name);
        request.push_str(": ");
        request.push_str(value);
        request.push_str("\r\n");
    }
    request.push_str("\r\n");
    request.push_str(body);
    stream.write_all(request.as_bytes()).map_err(io_error)?;
    let mut raw = String::new();
    stream.read_to_string(&mut raw).map_err(io_error)?;
    let (head, body) = raw.split_once("\r\n\r\n").unwrap_or((&raw, ""));
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(0);
    Ok(HttpResponse {
        status,
        body: body.to_string(),
    })
}

fn access_headers(access: Option<&Bound<'_, PyDict>>) -> PyResult<Vec<(String, String)>> {
    let Some(access) = access else {
        return Ok(Vec::new());
    };
    let Some(headers) = access.get_item("headers")? else {
        return Ok(Vec::new());
    };
    let headers = headers.downcast::<PyDict>()?;
    dict_to_env(Some(headers))
}

fn normalize_base_path(base_path: &str) -> String {
    let trimmed = base_path.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return "/".to_string();
    }
    format!("/{}", trimmed.trim_matches('/'))
}

fn join_url_path(base_path: &str, route: &str) -> String {
    let base_path = normalize_base_path(base_path);
    let route = format!("/{}", route.trim_start_matches('/'));
    if base_path == "/" {
        route
    } else {
        format!("{base_path}{route}")
    }
}

fn percent_encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn terminate_process(pid: u32) {
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .status();
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string()])
            .status();
    }
}

fn io_error(err: std::io::Error) -> PyErr {
    PyAgentError::new_err(err.to_string())
}

#[pymodule]
fn _clawblox(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyAgent>()?;
    m.add_class::<PyWorld>()?;
    m.add_class::<PyWorldStartResult>()?;
    m.add_class::<PyAgentStartResult>()?;
    m.add_class::<PyAgentStopResult>()?;
    Ok(())
}
