use crate::connection::{IrcConnector, IrcSink, IrcStream};
use futures::channel::mpsc;
use futures::prelude::*;
use futures::{join, select};
use minibot_byte_string::{ByteStr, ByteString};
use minibot_irc_raw::Message;

struct Sender<'a>(&'a mut IrcSink);

impl Sender<'_> {
    pub async fn send_n<T: IntoIterator<Item = S>, S: AsRef<[u8]>>(
        &mut self,
        cmd: &str,
        params: T,
    ) -> ClientResult<()> {
        self.0
            .send(Message::from_named_command_params(cmd, params))
            .await?;
        Ok(())
    }
}

fn join_bytes<T: IntoIterator<Item = S>, S: AsRef<[u8]>>(iter: T, connector: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    let mut first = true;
    for item in iter.into_iter() {
        if first {
            first = false;
        } else {
            result.extend(connector);
        }
        result.extend(item.as_ref());
    }
    result
}

#[derive(thiserror::Error, Debug)]
pub enum ClientError {
    #[error(transparent)]
    Connection(#[from] crate::connection::Error),

    #[error("Stream ended unexpectedly")]
    UnexpectedEnd,

    #[error(transparent)]
    Join(#[from] tokio::task::JoinError),

    #[error("Client has already been closed")]
    AlreadyClosed,

    #[error(transparent)]
    Irc(#[from] minibot_irc_raw::Error),
}

pub struct ClientFactory {
    connector: IrcConnector,
}

async fn initialize_irc_channel(
    user: &str,
    token: &str,
    irc_read: &mut IrcStream,
    irc_write: &mut IrcSink,
) -> ClientResult<()> {
    let mut irc_sender = Sender(irc_write);
    irc_sender.send_n("CAP", &["LS", "302"]).await?;

    let mut caps = Vec::new();
    loop {
        let message = irc_read.next().await.ok_or(ClientError::UnexpectedEnd)??;
        assert!(
            message.has_named_command("CAP"),
            "Unexpected message: {:?}",
            message
        );
        let params = message.params();
        assert!(params.len() >= 2);
        if params.len() == 2 {
            assert!(params[0].eq_bytes(b"LS"));
            let caps_list = &params[1];
            caps.extend(caps_list.split_spaces().map(ByteStr::to_byte_string));
        } else if params.len() == 3 {
            assert!(params[0].eq_bytes(b"*"));
            assert!(params[1].eq_bytes(b"LS"));
            let caps_list = &params[2];
            caps.extend(caps_list.split_spaces().map(ByteStr::to_byte_string));
            break;
        } else {
            panic!("Unexpected message: {:?}", message);
        }
    }

    eprintln!("Got caps: {:?}", caps);

    // Check that the caps are the expected set.

    let ack_args = join_bytes(caps, b" ");

    irc_sender.send_n("CAP", &[b"REQ", &ack_args[..]]).await?;

    let mut caps = Vec::new();
    loop {
        let message = irc_read.next().await.ok_or(ClientError::UnexpectedEnd)??;
        assert!(
            message.has_named_command("CAP"),
            "Unexpected message: {:?}",
            message
        );
        let params = message.params();
        assert!(params.len() >= 2);
        if params.len() == 2 {
            assert!(params[0].eq_bytes(b"ACK"));
            let caps_list = &params[1];
            caps.extend(caps_list.split_spaces().map(ByteStr::to_byte_string));
        } else if params.len() == 3 {
            assert!(params[0].eq_bytes(b"*"));
            assert!(params[1].eq_bytes(b"ACK"));
            let caps_list = &params[2];
            caps.extend(caps_list.split_spaces().map(ByteStr::to_byte_string));
            break;
        } else {
            panic!("Unexpected message: {:?}", message);
        }
    }

    irc_sender
        .send_n("PASS", &[&format!("oauth:{}", token)])
        .await?;
    irc_sender.send_n("NICK", &[user]).await?;
    irc_sender.send_n("CAP", &[b"END"]).await?;
    loop {
        let message = irc_read.next().await.ok_or(ClientError::UnexpectedEnd)??;
        if message.has_num_command(376) {
            break;
        }
    }

    Ok(())
}

async fn run_input_loop(
    mut input_stream: impl Stream<Item = Message> + Unpin,
    mut ping_stream: mpsc::Receiver<ByteString>,
    mut irc_write: IrcSink,
) {
    let mut read_op = input_stream.next().fuse();
    let mut ping_read_op = ping_stream.next().fuse();
    'outer: loop {
        select! {
            new_msg = read_op => {
                match new_msg {
                    Some(new_msg) => {
                        match irc_write.send(new_msg).await {
                            Ok(()) => {}
                            Err(_) => {
                              break 'outer;
                            }
                        }
                        read_op = input_stream.next().fuse();
                    }
                    None => break 'outer,
                }
            }
            new_ping = ping_read_op => {
                match new_ping {
                    Some(new_ping) => {
                        Message::from_named_command_params("PONG", &[&new_ping]);
                    }
                    None => break 'outer,
                }
            }
        };
    }
}

async fn run_output_loop(
    mut irc_read: IrcStream,
    mut ping_sink: mpsc::Sender<ByteString>,
    mut output_sink: mpsc::Sender<Message>,
) {
    while let Some(msg_or_err) = irc_read.next().await {
        match msg_or_err {
            Ok(msg) => {
                if msg.has_named_command("PING") {
                    if let Err(_) = ping_sink.send(msg.params()[0].to_byte_string()).await {
                        break;
                    }
                } else {
                    if let Err(_) = output_sink.send(msg).await {
                        break;
                    }
                }
            }
            Err(e) => {
                println!("{}", e);
                break;
            }
        }
    }
    let _ = join!(ping_sink.close(), output_sink.close());
}

impl ClientFactory {
    pub fn create() -> ClientResult<Self> {
        Ok(ClientFactory {
            connector: IrcConnector::new()?,
        })
    }

    pub async fn connect(
        &self,
        host: &str,
        port: u16,
        user: &str,
        token: &str,
    ) -> ClientResult<Client> {
        let (mut irc_read, mut irc_write) = self.connector.connect(host, port).await?;
        initialize_irc_channel(user, token, &mut irc_read, &mut irc_write).await?;
        Ok(Client::new(irc_read, irc_write))
    }
}

pub type ClientResult<T> = Result<T, ClientError>;

struct ClientInner {
    input: mpsc::Sender<Message>,
    handle: tokio::task::JoinHandle<()>,
}

pub struct Client(Option<ClientInner>);

impl Client {
    fn new(irc_read: IrcStream, irc_write: IrcSink) -> Self {
        let (input, input_stream) = mpsc::channel(3);
        let (output_sink, _) = mpsc::channel(3);

        let handle = tokio::spawn(async move {
            let input_stream =
                tokio::time::throttle(std::time::Duration::from_secs_f32(5.0 / 30.0), input_stream);
            let (ping_sink, ping_stream) = mpsc::channel(1);

            join! {
                run_input_loop(input_stream, ping_stream, irc_write),
                run_output_loop(irc_read, ping_sink, output_sink),
            };
        });

        Client(Some(ClientInner { input, handle }))
    }

    fn get_inner_mut(&mut self) -> ClientResult<&mut ClientInner> {
        self.0.as_mut().ok_or(ClientError::AlreadyClosed)
    }

    pub async fn close(mut self) -> ClientResult<()> {
        let ClientInner { handle, .. } = self.0.take().unwrap();
        handle.await?;
        Ok(())
    }

    async fn send_msg<T, S>(&mut self, command: &str, params: T) -> ClientResult<()>
    where
        T: IntoIterator<Item = S>,
        S: AsRef<[u8]>,
    {
        self.get_inner_mut()?
            .input
            .send(Message::from_named_command_params(command, params))
            .await
            .map_err(|_| ClientError::AlreadyClosed)?;
        Ok(())
    }

    pub async fn join(&mut self, channel: &str) -> ClientResult<()> {
        self.send_msg("JOIN", &[format!("#{}", channel)]).await
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        assert!(
            self.0.is_none(),
            "Client was dropped without being waited on."
        )
    }
}
