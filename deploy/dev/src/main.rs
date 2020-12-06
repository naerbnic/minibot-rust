mod config;

use std::borrow::Cow;
use std::ffi::OsString;
use std::process::Command;
use std::thread::{spawn, JoinHandle};


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

    let server_thread = spawn_cargo_run_server("minibot-server", "minibot-server", |cmd| {
        cmd.env("RUST_LOG", "INFO").args(&[
            "--dotenv",
            basedirs
                .find_config_file("config.env")
                .unwrap()
                .to_str()
                .unwrap(),
        ]);
    });

    server_thread.join().unwrap();
}
