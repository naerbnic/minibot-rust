mod config;

use std::borrow::Cow;
use std::ffi::OsString;
use std::process::Command;
use std::thread::{spawn, JoinHandle};
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(about = "Tool for initializing and running a dev environment")]
enum DevCommand {
    /// Runs the server. Assumes all associated services are running.
    Run {},
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

fn main() {
    env_logger::init();

    let basedirs = xdg::BaseDirectories::with_prefix("minibot-server").unwrap();

    let config: config::ConfigFile = toml::from_slice(
        &std::fs::read(
            basedirs
                .find_config_file("dev-config.toml")
                .expect("Expected to find config file."),
        )
        .unwrap(),
    )
    .unwrap();

    let twitch = config.oauth_configs.get("twitch").unwrap();

    let dev_command = DevCommand::from_args();

    match dev_command {
        DevCommand::Run { .. } => {
            let server_thread = spawn_cargo_run_server("minibot-server", "minibot-server", |cmd| {
                cmd.env("RUST_LOG", "INFO")
                    .env("MINIBOT_SERVER_ADDR", &config.minibot.address)
                    .env("MINIBOT_CLIENT_ID", &twitch.client.client_id)
                    .env("MINIBOT_CLIENT_SECRET", &twitch.client.client_secret)
                    .env("MINIBOT_REDIRECT_URL", &twitch.client.redirect_url);
            });

            server_thread.join().unwrap();
        }
    }
}
