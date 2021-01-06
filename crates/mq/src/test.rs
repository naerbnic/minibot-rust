use std::net::Ipv4Addr;

use docker_proc::{Error, PortProtocol, Process, Stdio};

struct TestMq {
    _process: Process,
}

impl TestMq {
    pub fn new() -> Result<Self, Error> {
        let proc = Process::builder("rabbitmq:3.8")
            .stdout(Stdio::new_line_waiter(&["Server startup complete;"]))
            .port(
                "main",
                5672,
                PortProtocol::Tcp,
                Ipv4Addr::LOCALHOST.into(),
                None,
            )
            .start()?;
        Ok(TestMq { _process: proc })
    }
}

#[test]
pub fn test_mq_test() -> Result<(), Error> {
    let _mq = TestMq::new()?;
    Ok(())
}
