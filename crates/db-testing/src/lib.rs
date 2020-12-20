pub mod docker;

use nix::sys::signal::kill;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use std::{convert::TryInto, path::Path};
use std::{
    io::{self, prelude::*},
    thread::{sleep, spawn},
};

use regex::Regex;

use tempdir::TempDir;

use minibot_db_postgres::DbHandle;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Db(#[from] minibot_db_postgres::Error),
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

fn get_docker_port(id: &str, expected_inner_port: u16) -> anyhow::Result<u16> {
    let output = Command::new("docker").arg("port").arg(id).output()?;

    anyhow::ensure!(
        output.status.success(),
        "docker port command failed: {:?}",
        output.status
    );

    let port_re = Regex::new(r"^(\d+)/([[:alpha:]]+) -> ([^:]+):(\d+)$").unwrap();

    for line in std::str::from_utf8(&output.stdout).unwrap().lines() {
        let cap = port_re.captures(line).unwrap();
        let inner_port: u16 = cap[1].parse()?;
        let _protocol = &cap[2];
        let _ext_hostname = &cap[3];
        let ext_port: u16 = cap[4].parse()?;

        if inner_port == expected_inner_port {
            return Ok(ext_port);
        }
    }

    anyhow::bail!(
        "Could not find docker binding for port {}",
        expected_inner_port
    );
}

pub struct TestDb {
    port: u16,
    password: Option<String>,
    process: docker::Process,
}

impl TestDb {
    pub fn new_docker() -> anyhow::Result<Self> {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);

        let process = docker::ProcessBuilder::new("postgres:13")
            .port(
                5432,
                docker::PortProtocol::Tcp,
                std::net::Ipv4Addr::LOCALHOST.into(),
                None,
            )
            .env("POSTGRES_PASSWORD", "postgres")
            .stdout(docker::StdIoHandler::new_line_func({
                let mut sender = Some(sender);
                move |line| {
                    if line.contains("ready for start up.") && sender.is_some() {
                        sender.take().unwrap().send(()).unwrap();
                    }
                }
            }))
            .exit_signal(docker::Signal::Quit)
            .start()?;

        let deadline = Instant::now() + Duration::from_secs(30);

        // Wait for the database to be ready
        let mut ext_port = None;
        for port in process.port_bindings()? {
            if port.internal_port() == 5432 {
                ext_port = Some(port.external_port());
                break;
            }
        }

        let ext_port = ext_port.unwrap();

        receiver.recv().unwrap();

        log::info!("Database started at port {}", ext_port);

        Ok(TestDb {
            port: ext_port,
            password: Some("postgres".to_string()),
            process,
        })
    }

    pub async fn handle(&self) -> Result<DbHandle, Error> {
        let url = match &self.password {
            Some(password) => {
                format!(
                    "postgres://postgres:{password}@127.0.0.1:{port}/postgres",
                    password = password,
                    port = self.port,
                )
            }
            None => {
                format!(
                    "postgres://postgres@127.0.0.1:{port}/postgres",
                    port = self.port,
                )
            }
        };
        Ok(DbHandle::new(&url).await?)
    }
}
