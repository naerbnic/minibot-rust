use std::{
    collections::{BTreeMap, HashMap},
    ffi::{OsStr, OsString},
    io::{self, BufRead},
    net::{IpAddr, Ipv4Addr},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Output, Stdio as ProcStdio},
    thread::{sleep, JoinHandle},
    time::{Duration, Instant},
};

use pinky_swear::{Pinky, PinkySwear};
use serde::Deserialize;
use tempdir::TempDir;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("A command failed with status: {0}")]
    CommandFailed(ExitStatus),
}

fn read_container_id(deadline: Instant, path: &Path) -> io::Result<String> {
    loop {
        if Instant::now() > deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Unable to open file before deadline.",
            ));
        }

        match std::fs::read_to_string(path) {
            Ok(str) => {
                if str.len() == 64 {
                    return Ok(str);
                }
            }
            Err(e) => {
                if !matches!(e.kind(), io::ErrorKind::NotFound) {
                    return Err(e);
                }
            }
        }

        sleep(Duration::from_millis(100));
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum PortProtocol {
    Tcp,
    Udp,
}

impl PortProtocol {
    fn as_str(&self) -> &'static str {
        match self {
            PortProtocol::Tcp => "tcp".as_ref(),
            PortProtocol::Udp => "udp".as_ref(),
        }
    }

    fn from_str(protocol_str: &str) -> Self {
        match protocol_str {
            "tcp" => PortProtocol::Tcp,
            "udp" => PortProtocol::Udp,
            _ => panic!("Unknown protocol: {}", protocol_str),
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
struct InternalPort {
    protocol: PortProtocol,
    port: u16,
}

impl InternalPort {
    pub fn to_arg(&self) -> String {
        format!("{}/{}", self.port, self.protocol.as_str())
    }
}

#[derive(Copy, Clone)]
struct PortMapping {
    internal_port: InternalPort,
    interface: IpAddr,
    external_port: Option<u16>,
}

impl PortMapping {
    pub fn as_arg(&self) -> OsString {
        let mut arg = OsString::new();
        arg.push(format!("{}", self.interface));
        arg.push(":");
        if let Some(port) = self.external_port {
            arg.push(format!("{}", port));
        }
        arg.push(":");
        arg.push(self.internal_port.to_arg());
        arg
    }
}

#[derive(Clone)]
struct Mount {
    destination: OsString,
    read_only: bool,
    source: OsString,
}

impl Mount {
    fn as_mount(&self) -> OsString {
        let mut mount = OsString::new();
        mount.push("type=volume,destination=");
        mount.push(&self.destination);
        mount.push(",source=");
        mount.push(&self.source);
        if self.read_only {
            mount.push(",readonly");
        }
        mount
    }
}

enum StdIoHandlerInner {
    DropData,
    LineReader(Box<dyn FnMut(&str) + Send + 'static>),
    LineWaiter(Vec<String>),
}

impl StdIoHandlerInner {
    fn handle_stream(self, mut stream: impl io::Read, ready: Pinky<()>) -> io::Result<()> {
        match self {
            StdIoHandlerInner::DropData => {
                ready.swear(());
                let mut buffer = [0u8; 32 * 1024];
                loop {
                    let bytes_read = stream.read(&mut buffer)?;
                    if bytes_read == 0 {
                        break Ok(());
                    }
                }
            }
            StdIoHandlerInner::LineReader(mut handler) => {
                ready.swear(());
                let stream = io::BufReader::new(stream);
                for line in stream.lines() {
                    let line = line?;
                    handler(&line);
                }
                Ok(())
            }
            StdIoHandlerInner::LineWaiter(lines) => {
                let stream = io::BufReader::new(stream);
                let mut curr_index = 0;
                for line in stream.lines() {
                    let line = line?;
                    if curr_index < lines.len() && line.contains(&lines[curr_index]) {
                        curr_index += 1;
                        if curr_index == lines.len() {
                            ready.swear(())
                        }
                    }
                }
                Ok(())
            }
        }
    }
}

pub struct Stdio(StdIoHandlerInner);

impl Stdio {
    pub fn new_drop_data() -> Self {
        Stdio(StdIoHandlerInner::DropData)
    }

    pub fn new_line_func<F>(func: F) -> Self
    where
        F: FnMut(&str) + Send + 'static,
    {
        Stdio(StdIoHandlerInner::LineReader(Box::new(func)))
    }

    pub fn new_line_waiter(lines: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        Stdio(StdIoHandlerInner::LineWaiter(
            lines.into_iter().map(|i| i.as_ref().to_string()).collect(),
        ))
    }

    fn handle_stream(self, stream: impl io::Read, ready: Pinky<()>) -> io::Result<()> {
        self.0.handle_stream(stream, ready)
    }
}

#[derive(Copy, Clone)]
pub enum Signal {
    Kill,
    Term,
    Int,
    Quit,
    HangUp,
}

impl Signal {
    fn as_signal_name(&self) -> &'static str {
        match self {
            Signal::Kill => "SIGKILL",
            Signal::Term => "SIGTERM",
            Signal::Int => "SIGINT",
            Signal::Quit => "SIGQUIT",
            Signal::HangUp => "SIGHUP",
        }
    }
}

pub struct ProcessBuilder {
    image: OsString,
    ports: HashMap<String, PortMapping>,
    mounts: Vec<Mount>,
    args: Vec<OsString>,
    env: BTreeMap<OsString, OsString>,
    stdout: Stdio,
    stderr: Stdio,
    exit_signal: Signal,
}

impl ProcessBuilder {
    fn new(image: impl AsRef<OsStr>) -> Self {
        ProcessBuilder {
            image: image.as_ref().to_os_string(),
            ports: HashMap::new(),
            mounts: Vec::new(),
            args: Vec::new(),
            env: BTreeMap::new(),
            stdout: Stdio::new_drop_data(),
            stderr: Stdio::new_drop_data(),
            exit_signal: Signal::Kill,
        }
    }

    pub fn port(
        &mut self,
        name: &str,
        internal_port: u16,
        protocol: PortProtocol,
        interface: IpAddr,
        external_port: Option<u16>,
    ) -> &mut Self {
        self.ports.insert(
            name.to_string(),
            PortMapping {
                internal_port: InternalPort {
                    port: internal_port,
                    protocol,
                },
                interface,
                external_port,
            },
        );
        self
    }

    pub fn volume(
        &mut self,
        source: impl AsRef<OsStr>,
        destination: impl AsRef<OsStr>,
        read_only: bool,
    ) -> &mut Self {
        self.mounts.push(Mount {
            destination: destination.as_ref().to_os_string(),
            read_only: read_only,
            source: source.as_ref().to_os_string(),
        });
        self
    }

    pub fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Self {
        self.args.push(arg.as_ref().to_os_string());
        self
    }

    pub fn env(&mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> &mut Self {
        // Check that the environment is a valid identifier
        let key = key.as_ref();
        assert!(key
            .to_str()
            .unwrap()
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_'));
        self.env
            .insert(key.to_os_string(), value.as_ref().to_os_string());
        self
    }

    pub fn stdout(&mut self, handler: Stdio) -> &mut Self {
        self.stdout = handler;
        self
    }

    pub fn stderr(&mut self, handler: Stdio) -> &mut Self {
        self.stderr = handler;
        self
    }

    pub fn exit_signal(&mut self, signal: Signal) -> &mut Self {
        self.exit_signal = signal;
        self
    }

    pub fn start(&mut self) -> Result<Process, Error> {
        let tmp_dir = TempDir::new("db")?;
        let container_id_file = tmp_dir.path().join("container_id");
        let mut cmd = Command::new("docker");

        // Set up common arguments
        cmd
            // The `run` command actuall executes the image
            .arg("run")
            // We want an interactive session. Ensures that the command won't end until the process
            // ends, even if stdin is closed.
            .arg("-i")
            // Remove the container after it exits.
            .arg("--rm")
            // Run with an internal init process. This ensures correct handling of signals
            .arg("--init")
            // Signals sent to the docker process will be proxied to the containerized process.
            .arg("--sig-proxy=true")
            // Writes the container ID to a file, so we can further manipulate it.
            .args(&["--cidfile", container_id_file.to_str().unwrap()])
            // We assume this is a server process, so we don't use stdin here.
            .stdin(ProcStdio::null())
            // Both stdout and stderr can be useful for ready checking and error checking, so we
            // pipe them
            .stdout(ProcStdio::piped())
            .stderr(ProcStdio::piped());

        for (_, mapping) in &self.ports {
            cmd.arg("-p").arg(&mapping.as_arg());
        }

        for mount in &self.mounts {
            cmd.arg("--mount").arg(&mount.as_mount());
        }

        for (k, v) in &self.env {
            let mut env_arg = OsString::new();
            env_arg.push(k);
            env_arg.push("=");
            env_arg.push(v);
            cmd.arg("-e").arg(&env_arg);
        }

        cmd.arg(&self.image);

        for arg in &self.args {
            cmd.arg(arg);
        }

        let mut process = cmd.spawn()?;

        let stdout = process.stdout.take().expect("stdout was piped");
        let stderr = process.stderr.take().expect("stderr was piped");

        let (stdout_wait, stdout_ready) = PinkySwear::new();
        let (stderr_wait, stderr_ready) = PinkySwear::new();

        let stdout_thread = std::thread::spawn({
            let stdout_handler = std::mem::replace(&mut self.stdout, Stdio::new_drop_data());
            move || {
                stdout_handler.handle_stream(stdout, stdout_ready).unwrap();
            }
        });

        let stderr_thread = std::thread::spawn({
            let stderr_handler = std::mem::replace(&mut self.stderr, Stdio::new_drop_data());
            move || {
                stderr_handler.handle_stream(stderr, stderr_ready).unwrap();
            }
        });

        let container_id = read_container_id(
            Instant::now() + Duration::from_secs(100),
            &container_id_file,
        )?;

        stdout_wait.wait();
        stderr_wait.wait();

        let port_to_names: HashMap<InternalPort, String> = self
            .ports
            .iter()
            .map(|(name, p)| (p.internal_port, name.clone()))
            .collect();

        let port_bindings = get_container_port_bindings(&container_id)?
            .into_iter()
            .map(|p| (port_to_names.get(&p.internal_port()).unwrap().clone(), p))
            .collect();

        Ok(Process {
            process: Some(process),
            container_id,
            stdout_thread: Some(stdout_thread),
            stderr_thread: Some(stderr_thread),
            ports: port_bindings,
            exit_signal: self.exit_signal,
        })
    }
}

#[derive(Copy, Clone, Debug)]
pub struct PortBinding {
    internal_port: InternalPort,
    interface: IpAddr,
    external_port: u16,
}

impl PortBinding {
    fn from_inner_binding(inner_spec: &str, host_ip_str: &str, host_port_str: &str) -> Self {
        let parts = inner_spec.split('/').collect::<Vec<_>>();
        assert_eq!(parts.len(), 2);
        let internal_port_str = parts[0];
        let protocol_str = parts[1];

        let internal_port: u16 = internal_port_str
            .parse()
            .expect("Docker has correct format.");
        let protocol = PortProtocol::from_str(protocol_str);

        let interface: IpAddr = if host_ip_str.is_empty() {
            Ipv4Addr::UNSPECIFIED.into()
        } else {
            host_ip_str.parse().unwrap()
        };

        let external_port = host_port_str.parse().unwrap();

        PortBinding {
            internal_port: InternalPort {
                port: internal_port,
                protocol,
            },
            interface,
            external_port,
        }
    }

    fn internal_port(&self) -> InternalPort {
        self.internal_port
    }

    pub fn interface(&self) -> &IpAddr {
        &self.interface
    }

    pub fn external_port(&self) -> u16 {
        self.external_port
    }
}

#[derive(Deserialize)]
struct PortBindingInner {
    #[serde(rename = "HostIp")]
    host_ip: String,
    #[serde(rename = "HostPort")]
    host_port: String,
}

pub struct Process {
    process: Option<Child>,
    container_id: String,
    stdout_thread: Option<JoinHandle<()>>,
    stderr_thread: Option<JoinHandle<()>>,
    ports: HashMap<String, PortBinding>,
    exit_signal: Signal,
}

fn run_docker_command<F>(config_func: F) -> io::Result<Output>
where
    F: FnOnce(&mut Command),
{
    let mut cmd = Command::new("docker");
    config_func(&mut cmd);
    cmd.stdin(ProcStdio::null())
        .stdout(ProcStdio::piped())
        .stderr(ProcStdio::piped())
        .output()
}

pub fn get_container_port_bindings(container_id: &str) -> Result<Vec<PortBinding>, Error> {
    let output = run_docker_command(|cmd| {
        cmd.arg("container")
            .arg("inspect")
            .args(&["--format", "{{json .NetworkSettings.Ports}}"])
            .arg(container_id);
    })?;

    if !output.status.success() {
        return Err(Error::CommandFailed(output.status.clone()));
    }

    let bindings = serde_json::from_slice::<BTreeMap<String, serde_json::Value>>(&output.stdout)
        .expect("Docker should produce valid json");

    Ok(bindings
        .into_iter()
        .filter_map(|(k, v)| {
            if v.is_null() {
                None
            } else {
                Some(serde_json::from_value::<Vec<PortBindingInner>>(v).map(move |bind| (k, bind)))
            }
        })
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
        .into_iter()
        .flat_map(|(k, v)| {
            v.into_iter()
                .map(|inner_binding| (k.clone(), inner_binding))
                .collect::<Vec<_>>()
        })
        .map(|(k, inner_binding)| {
            PortBinding::from_inner_binding(&k, &inner_binding.host_ip, &inner_binding.host_port)
        })
        .collect())
}

impl Process {
    pub fn builder(image: impl AsRef<OsStr>) -> ProcessBuilder {
        ProcessBuilder::new(image)
    }

    pub fn port_address(&self, name: &str) -> Option<PortBinding> {
        self.ports.get(name).copied()
    }

    /// Returns an [`ExecBuilder`](ExecBuilder) that will run a program within the container.
    pub fn build_exec<'a>(&'a self, command: impl AsRef<OsStr>) -> ExecBuilder<'a> {
        ExecBuilder {
            process: self,
            binary: command.as_ref().to_os_string(),
            workdir: None,
            args: Vec::new(),
            env: BTreeMap::new(),
        }
    }

    /// Consumes and exits this container process.
    pub fn exit(mut self) -> io::Result<()> {
        self.inner_exit()
    }
}

/// Inner helpers
impl Process {
    fn run_docker_command<F>(&self, config_func: F) -> io::Result<Output>
    where
        F: FnOnce(&mut Command),
    {
        let mut cmd = Command::new("docker");
        config_func(&mut cmd);
        cmd.stdin(ProcStdio::null())
            .stdout(ProcStdio::piped())
            .stderr(ProcStdio::piped())
            .output()
    }

    fn inner_exit(&mut self) -> io::Result<()> {
        if let Some(mut process) = self.process.take() {
            Command::new("docker")
                .arg("kill")
                .arg(format!(
                    "--signal={signal}",
                    signal = self.exit_signal.as_signal_name()
                ))
                .arg(&self.container_id)
                .stdout(ProcStdio::null())
                .stderr(ProcStdio::null())
                .status()?;
            process.wait()?;
            self.stdout_thread.take().unwrap().join().unwrap();
            self.stderr_thread.take().unwrap().join().unwrap();
        }
        Ok(())
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        // Ignore the output error
        let _ = self.inner_exit();
    }
}

pub struct ExecBuilder<'a> {
    process: &'a Process,
    binary: OsString,
    workdir: Option<PathBuf>,
    args: Vec<OsString>,
    env: BTreeMap<OsString, OsString>,
}

impl ExecBuilder<'_> {
    /// Sets the working directory the program will be run on within the
    pub fn workdir(&mut self, path: impl AsRef<Path>) -> &mut Self {
        self.workdir = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Self {
        self.args.push(arg.as_ref().to_os_string());
        self
    }

    pub fn env(&mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> &mut Self {
        self.env
            .insert(key.as_ref().to_os_string(), value.as_ref().to_os_string());
        self
    }

    pub fn exec(&mut self) -> io::Result<Output> {
        self.process.run_docker_command(|cmd| {
            cmd.arg("exec").arg("-i");
            if let Some(workdir) = &self.workdir {
                cmd.arg("--workdir").arg(workdir.as_os_str());
            }
    
            for (k, v) in &self.env {
                let mut var = OsString::new();
                var.push(k);
                var.push("=");
                var.push(v);
                cmd.arg("-e");
                cmd.arg(&var);
            }
    
            cmd.arg(&self.process.container_id).arg(&self.binary);
    
            for arg in &self.args {
                cmd.arg(arg);
            }
        })
    }
}
