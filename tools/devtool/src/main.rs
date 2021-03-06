mod migrations;

use minibot_config::{PgDbType, PgUserType, PostgresDev};
use std::borrow::Cow;
use std::ffi::OsString;
use std::process::Command;
use std::thread::{spawn, JoinHandle};
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(about = "Tool for initializing and running a dev environment")]
struct Arguments {
    /// The path to the `deploy/dev` directory
    #[structopt(long)]
    script_home: String,
    #[structopt(subcommand)]
    subcommand: DevCommand,
}

#[derive(StructOpt)]
enum DevCommand {
    /// Runs the server. Assumes all associated services are running.
    Run,
    /// Creates the database. Does not initialize it in any way
    PgCreateDb,
    /// Drops the database. Will ask for confirmation.
    PgDropDb,
    /// Starts an interactive psql session with the database, logged in as the client user.
    PgSql,
    /// Applies migrations to the database.
    ApplyMigrations,
    /// Resets the database by dropping, creating, then resetting the database.
    PgResetDb,
}

fn new_cargo_run_command<'a>(
    package: impl Into<Cow<'a, str>>,
    bin: impl Into<Cow<'a, str>>,
) -> Command {
    let mut cmd = Command::new(std::env::var_os("CARGO").unwrap_or(OsString::from("cargo")));
    cmd.arg("run")
        .args(&["--package", package.into().as_ref()])
        .args(&["--bin", bin.into().as_ref()])
        .arg("--");
    cmd
}

fn run_command(cmd: impl AsRef<str>, config: impl FnOnce(&mut Command)) {
    let mut child = Command::new(cmd.as_ref());
    config(&mut child);
    child.spawn().unwrap().wait().unwrap();
}

fn spawn_server(mut cmd: Command) -> JoinHandle<()> {
    spawn(move || {
        let mut child = cmd.spawn().unwrap();
        child.wait().unwrap();
    })
}

fn spawn_cargo_run_server<'a>(
    package: impl Into<Cow<'a, str>>,
    bin: impl Into<Cow<'a, str>>,
    config: impl FnOnce(&mut Command),
) -> JoinHandle<()> {
    let mut cmd = new_cargo_run_command(package, bin);
    config(&mut cmd);
    spawn_server(cmd)
}

fn create_db(pg: &PostgresDev) {
    run_command("createdb", |cmd| {
        cmd.env("PGPASSWORD", &pg.admin_user.password)
            .env("PGDATABASE", &pg.db_name)
            .env("PGHOST", &pg.hostname)
            .env("PGPORT", pg.port.to_string())
            .env("PGUSER", &pg.admin_user.username)
            .args(&["--owner", &pg.client_user.username])
            .arg("--no-password");
    });
}

fn drop_db(pg: &PostgresDev) {
    run_command("dropdb", |cmd| {
        cmd.env("PGPASSWORD", &pg.admin_user.password)
            .env("PGHOST", &pg.hostname)
            .env("PGPORT", pg.port.to_string())
            .env("PGUSER", &pg.admin_user.username)
            .arg("--no-password")
            .arg("--interactive")
            .arg(&pg.db_name);
    });
}

fn main() {
    env_logger::init();

    let basedirs = xdg::BaseDirectories::with_prefix("minibot-server").unwrap();

    let config: minibot_config::ConfigFile = toml::from_slice(
        &std::fs::read(
            basedirs
                .find_config_file("dev-config.toml")
                .expect("Expected to find config file."),
        )
        .unwrap(),
    )
    .unwrap();

    let twitch = config.oauth_configs.get("twitch").unwrap();

    let arguments = Arguments::from_args();

    match &arguments.subcommand {
        DevCommand::Run => {
            let server_thread = spawn_cargo_run_server("minibot-server", "minibot-server", |cmd| {
                cmd.current_dir(&arguments.script_home)
                    .env("RUST_LOG", "INFO")
                    .env("MINIBOT_SERVER_ADDR", &config.minibot.address)
                    .env("MINIBOT_CLIENT_ID", twitch.client.client_id())
                    .env("MINIBOT_CLIENT_SECRET", twitch.client.client_secret())
                    .env("MINIBOT_REDIRECT_URL", twitch.client.redirect_url())
                    .env(
                        "MINIBOT_TWITCH_CLIENT",
                        &minibot_config::fmt::to_string(&twitch.client).unwrap(),
                    );
            });

            server_thread.join().unwrap();
        }

        DevCommand::PgCreateDb => create_db(&config.postgres),
        DevCommand::PgDropDb => drop_db(&config.postgres),

        DevCommand::PgSql => run_command("psql", |cmd| {
            let pg = &config.postgres;
            cmd.arg(
                pg.db_config(PgUserType::Client, PgDbType::Main)
                    .connection_url(),
            );
        }),

        DevCommand::ApplyMigrations => migrations::apply_migrations(
            &config
                .postgres
                .db_config(PgUserType::Client, PgDbType::Main)
                .connection_url(),
        ),

        DevCommand::PgResetDb => {
            drop_db(&config.postgres);
            create_db(&config.postgres);
            migrations::apply_migrations(
                &config
                    .postgres
                    .db_config(PgUserType::Client, PgDbType::Main)
                    .connection_url(),
            );
        }
    }
}
