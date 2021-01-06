use std::net::{Ipv4Addr, SocketAddr};

use docker_proc::{PortProtocol, Process, Signal, Stdio};
use minibot_db_postgres::DbHandle;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Db(#[from] minibot_db_postgres::Error),
}

pub struct TestDb {
    addr: SocketAddr,
    password: String,
    _process: Process,
}

impl TestDb {
    pub fn new_docker() -> anyhow::Result<Self> {
        let password = "postgres";
        let process = Process::builder("postgres:13")
            .port(
                "main",
                5432,
                PortProtocol::Tcp,
                Ipv4Addr::LOCALHOST.into(),
                None,
            )
            .env("POSTGRES_PASSWORD", password)
            .stdout(Stdio::new_line_waiter(&["ready for start up"]))
            .exit_signal(Signal::Quit)
            .start()?;

        // Wait for the database to be ready
        let sock_addr = process.port_address("main").unwrap();

        log::info!("Database started at addr {}", sock_addr);

        Ok(TestDb {
            addr: sock_addr,
            password: "postgres".to_string(),
            _process: process,
        })
    }

    pub async fn handle(&self) -> Result<DbHandle, Error> {
        let url = format!(
            "postgres://postgres:{password}@{addr}/postgres",
            password = self.password,
            addr = self.addr,
        );
        Ok(DbHandle::new(&url).await?)
    }
}
