use docker_process::{PortProtocol, Process, Signal, Stdio};
use minibot_db_postgres::DbHandle;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Db(#[from] minibot_db_postgres::Error),
}

pub struct TestDb {
    port: u16,
    password: Option<String>,
    _process: Process,
}

impl TestDb {
    pub fn new_docker() -> anyhow::Result<Self> {
        let process = Process::builder("postgres:13")
            .port(
                5432,
                PortProtocol::Tcp,
                std::net::Ipv4Addr::LOCALHOST.into(),
                None,
            )
            .env("POSTGRES_PASSWORD", "postgres")
            .stdout(Stdio::new_line_waiter(&["ready_for_start_up"]))
            .exit_signal(Signal::Quit)
            .start()?;

        // Wait for the database to be ready
        let mut ext_port = None;
        for port in process.port_bindings()? {
            if port.internal_port() == 5432 {
                ext_port = Some(port.external_port());
                break;
            }
        }

        let ext_port = ext_port.unwrap();

        log::info!("Database started at port {}", ext_port);

        Ok(TestDb {
            port: ext_port,
            password: Some("postgres".to_string()),
            _process: process,
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
