use std::net::Ipv4Addr;

use futures::stream::StreamExt;

use docker_proc::{PortProtocol, Process, Stdio};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Lapin(#[from] lapin::Error),
    #[error(transparent)]
    DockerProc(#[from] docker_proc::Error),
}
struct TestBroker {
    process: Process,
}

impl TestBroker {
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

        Ok(TestBroker {
            process: proc,
        })
    }

    pub fn url(&self) -> String {
        format!(
            "amqp://guest:guest@{addr}/%2F",
            addr = self.process.port_address("main").unwrap()
        )
    }
}

#[tokio::test]
pub async fn test_mq_test() -> anyhow::Result<()> {
    let mq = TestBroker::new()?;

    let broker = crate::Broker::new(&mq.url()).await?;
    let source = crate::MessageSource::User("alice".to_string());

    let queue = broker.create_queue(&source, std::time::Duration::from_secs(60)).await?;

    eprintln!("Created queue with id: {:?}", queue.id());

    let queue_id = queue.id().clone();
    let mut queue_stream = queue.into_stream();

    broker.send_message(&source, crate::Message::new("Hello, World!".as_bytes())).await?;
    broker.send_message(&source, crate::Message::new("Goodbye, World!".as_bytes())).await?;

    let msg = queue_stream.next().await.unwrap();
    assert_eq!(msg.data(), "Hello, World!".as_bytes());

    drop(queue_stream);

    let mut queue_stream = broker.open_queue(&queue_id).await?.into_stream();
    let msg = queue_stream.next().await.unwrap();

    assert_eq!(msg.data(), "Goodbye, World!".as_bytes());
    Ok(())
}
