use std::{
    collections::BTreeMap,
    io::{self, BufRead},
    net::IpAddr,
    path::Path,
    process::{Child, Command, Stdio},
    thread::{sleep, JoinHandle},
    time::{Duration, Instant},
};

use tempdir::TempDir;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
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

#[derive(Copy, Clone, Debug)]
pub enum PortProtocol {
    Tcp,
    Udp,
}

impl PortProtocol {
    fn as_str(&self) -> &'static str {
        match self {
            PortProtocol::Tcp => "tcp",
            PortProtocol::Udp => "udp",
        }
    }
}

#[derive(Clone)]
struct PortMapping {
    interface: IpAddr,
    protocol: PortProtocol,
    internal_port: u16,
    external_port: Option<u16>,
}

impl PortMapping {
    pub fn as_arg(&self) -> String {
        format!(
            "{addr}:{external_port}:{internal_port}/{port_protocol}",
            addr = self.interface,
            external_port = if let Some(port) = self.external_port {
                format!("{}", port)
            } else {
                "".to_string()
            },
            internal_port = self.internal_port,
            port_protocol = self.protocol.as_str(),
        )
    }
}

trait LineReaderFunc: Send {
    fn clone_func(&self) -> Box<dyn LineReaderFunc>;
    fn call(&mut self, line: &str);
}

impl<F> LineReaderFunc for F
where
    F: FnMut(&str) + Clone + Send + 'static,
{
    fn clone_func(&self) -> Box<dyn LineReaderFunc> {
        Box::new(self.clone())
    }

    fn call(&mut self, line: &str) {
        self(line)
    }
}

enum StdIoHandlerInner {
    DropData,
    LineReader(Box<dyn LineReaderFunc>),
}

impl Clone for StdIoHandlerInner {
    fn clone(&self) -> Self {
        match self {
            StdIoHandlerInner::DropData => StdIoHandlerInner::DropData,
            StdIoHandlerInner::LineReader(func) => StdIoHandlerInner::LineReader(func.clone_func()),
        }
    }
}

impl StdIoHandlerInner {
    fn handle_stream(&mut self, mut stream: impl io::Read) -> io::Result<()> {
        match self {
            StdIoHandlerInner::DropData => {
                let mut buffer = [0u8; 32 * 1024];
                loop {
                    let bytes_read = stream.read(&mut buffer)?;
                    if bytes_read == 0 {
                        break Ok(());
                    }
                }
            }
            StdIoHandlerInner::LineReader(handler) => {
                let stream = io::BufReader::new(stream);
                for line in stream.lines() {
                    let line = line?;
                    handler.call(&line);
                }
                Ok(())
            }
        }
    }
}

#[derive(Clone)]
pub struct StdIoHandler(StdIoHandlerInner);

impl StdIoHandler {
    pub fn new_drop_data() -> Self {
        StdIoHandler(StdIoHandlerInner::DropData)
    }

    pub fn new_line_func<F>(func: F) -> Self
    where
        F: FnMut(&str) + Clone + Send + 'static,
    {
        StdIoHandler(StdIoHandlerInner::LineReader(Box::new(func)))
    }

    fn handle_stream(&mut self, stream: impl io::Read) -> io::Result<()> {
        self.0.handle_stream(stream)
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

#[derive(Clone)]
pub struct ProcessBuilder {
    image: String,
    ports: Vec<PortMapping>,
    args: Vec<String>,
    env: BTreeMap<String, String>,
    stdout: StdIoHandler,
    stderr: StdIoHandler,
    exit_signal: Signal,
}

impl ProcessBuilder {
    pub fn new<'a>(image: impl Into<std::borrow::Cow<'a, str>>) -> Self {
        ProcessBuilder {
            image: image.into().into_owned(),
            ports: Vec::new(),
            args: Vec::new(),
            env: BTreeMap::new(),
            stdout: StdIoHandler::new_drop_data(),
            stderr: StdIoHandler::new_drop_data(),
            exit_signal: Signal::Kill,
        }
    }

    pub fn port(&mut self, internal_port: u16, protocol: PortProtocol, interface: IpAddr, external_port: Option<u16>) -> &mut Self {
        self.ports.push(PortMapping {
            internal_port,
            protocol,
            interface,
            external_port,
        });
        self
    }

    pub fn start(&self) -> Result<Process, Error> {
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
            .stdin(Stdio::null())
            // Both stdout and stderr can be useful for ready checking and error checking, so we
            // pipe them
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for port in &self.ports {
            cmd.args(&["-p", &port.as_arg()]);
        }

        for (k, v) in &self.env {
            cmd.args(&["-e", &format!("{key}={value}", key = k, value = v)]);
        }

        cmd.arg(&self.image);

        for arg in &self.args {
            cmd.arg(arg);
        }

        let mut process = cmd.spawn()?;

        let stdout = process.stdout.take().expect("stdout was piped");
        let stderr = process.stdout.take().expect("stderr was piped");

        let stdout_thread = std::thread::spawn({
            let mut stdout_handler = self.stdout.clone();
            move || {
                stdout_handler.handle_stream(stdout).unwrap();
            }
        });

        let stderr_thread = std::thread::spawn({
            let mut stderr_handler = self.stderr.clone();
            move || {
                stderr_handler.handle_stream(stderr).unwrap();
            }
        });

        let container_id =
            read_container_id(Instant::now() + Duration::from_secs(1), &container_id_file)?;

        Ok(Process {
            process: Some(process),
            container_id,
            stdout_thread: Some(stdout_thread),
            stderr_thread: Some(stderr_thread),
            exit_signal: self.exit_signal,
        })
    }
}

pub struct Process {
    process: Option<Child>,
    container_id: String,
    stdout_thread: Option<JoinHandle<()>>,
    stderr_thread: Option<JoinHandle<()>>,
    exit_signal: Signal,
}

impl Process {

}

/// Inner helpers
impl Process {
    fn inner_exit(&mut self) -> io::Result<()> {
        if let Some(mut process) = self.process.take() {
            Command::new("docker")
                .arg("kill")
                .arg(format!(
                    "--signal={signal}",
                    signal = self.exit_signal.as_signal_name()
                ))
                .arg(&self.container_id)
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

pub struct PortBinding {
    
}

pub struct ExecCommand {

}
