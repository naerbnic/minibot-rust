use nix::sys::signal::kill;
use std::convert::TryInto;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use tempdir::TempDir;

use minibot_db_postgres::DbHandle;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Db(#[from] minibot_db_postgres::Error),
}

pub struct TestDb {
    // Kept for RAII killing of the database process
    #[allow(dead_code)]
    tmp_dir: TempDir,
    uds_dir: PathBuf,
    password: Option<String>,
    postgres_process: Child,
}

impl TestDb {
    pub fn new_docker() -> anyhow::Result<Self> {
        let tmp_dir = TempDir::new("db")?;
        let uid = nix::unistd::geteuid();
        let gid = nix::unistd::getegid();
        nix::unistd::chown(tmp_dir.path(), Some(uid), Some(gid)).unwrap();

        let data_dir = tmp_dir.path().join("data");
        std::fs::create_dir(&data_dir)?;
        let uds_dir = tmp_dir.path().join("sock");
        std::fs::create_dir(&uds_dir)?;
        let uds_path = uds_dir.join(".s.PGSQL.5432");

        let mut cmd = Command::new("docker");
        cmd.arg("run")
            .arg("-i")
            .arg("--rm")
            .arg("--init")
            .arg("--sig-proxy=true")
            .args(&[
                "--user",
                &format!("{}:{}", dbg!(uid.as_raw()), dbg!(gid.as_raw())),
            ])
            .args(&["-e", "POSTGRES_PASSWORD=postgres"])
            .args(&[
                "-v",
                &format!(
                    "{}:/var/lib/postgresql/data:rw",
                    &data_dir.to_string_lossy()
                ),
            ])
            .args(&[
                "-v",
                &format!("{}:/var/lib/postgresql/sock:rw", &uds_dir.to_string_lossy()),
            ])
            .arg("postgres:13")
            .args(&["-h", ""])
            .args(&["-k", "/var/lib/postgresql/sock"])
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        dbg!(&cmd);

        let postgres_process = cmd.spawn()?;

        eprintln!("PID: {}", postgres_process.id());

        // Wait for the database to be ready

        //std::thread::sleep(std::time::Duration::from_secs(5));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);

        let success = loop {
            if std::time::Instant::now() >= deadline {
                break false;
            }
            if dbg!(&uds_path).exists() {
                break true;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        };

        if !success {
            eprintln!("Unable to connect to socket.");
            kill(
                nix::unistd::Pid::from_raw(postgres_process.id().try_into().unwrap()),
                nix::sys::signal::Signal::SIGTERM,
            )
            .unwrap();
            anyhow::bail!("Unable to connect to database.");
        } else {
            eprintln!("Discovered socket.");
        }

        Ok(TestDb {
            tmp_dir,
            uds_dir,
            password: Some("postgres".to_string()),
            postgres_process,
        })
    }

    pub async fn handle(&self) -> Result<DbHandle, Error> {
        let url = match &self.password {
            Some(password) => {
                format!(
                    "postgres:///postgres?host={uds_dir}&user=postgres&password={password}",
                    uds_dir = self.uds_dir.to_string_lossy(),
                    password = password,
                )
            }
            None => {
                format!(
                    "postgres:///postgres?host={uds_dir}&user=postgres",
                    uds_dir = self.uds_dir.to_string_lossy(),
                )
            }
        };
        Ok(DbHandle::new(dbg!(&url)).await?)
    }
}

impl Drop for TestDb {
    fn drop(&mut self) {
        kill(
            nix::unistd::Pid::from_raw(self.postgres_process.id().try_into().unwrap()),
            nix::sys::signal::Signal::SIGTERM,
        )
        .unwrap();
        self.postgres_process.wait().unwrap();
    }
}
