extern crate anyhow;
extern crate futures;
extern crate minibot_irc;
extern crate tokio;

use futures::prelude::*;
use minibot_irc::messages::{Command, Message};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let ds = devsecrets::DevSecrets::from_id(&devsecrets::import_id!()).unwrap();
    let key = ds.read_from("irc_key.txt").to_string().unwrap();

    if false {
        let messages = vec![
            Message::from_command_params(Command::from_name("CAP"), &["LS", "302"]),
            Message::from_command_params(Command::from_name("PASS"), &[&format!("oauth:{}", key)]),
            Message::from_command_params(Command::from_name("NICK"), &["ludofex"]),
            Message::from_command_params(
                Command::from_name("CAP"),
                &[
                    "REQ",
                    "twitch.tv/tags twitch.tv/commands twitch.tv/membership",
                ],
            ),
            Message::from_command_params(Command::from_name("CAP"), &["END"]),
            Message::from_command_params(Command::from_name("JOIN"), &["#ludofex"]),
            Message::from_command_params(Command::from_name("JOIN"), &["#ludofex"]),
            Message::from_command_params(
                Command::from_name("PRIVMSG"),
                &["#ludofex", "Hello, World!"],
            ),
            Message::from_command_params(Command::from_name("PRIVMSG"), &["#ludofex", "/me waves"]),
        ];

        let connector = minibot_irc::connection::IrcConnector::new()?;
        let (read, mut write) = connector.connect("irc.chat.twitch.tv", 6697).await?;
        println!("Connected and streams created.");
        write
            .send_all(&mut stream::iter(messages).map(|x| Ok(x)))
            .await?;
        println!("PASS sent.");
        let stream_start = tokio::time::Instant::now();
        read.try_for_each(|msg| async move {
            println!("{:?}: Server Msg: {:?}", stream_start.elapsed(), msg);
            Ok(())
        })
        .await?;
    } else {
        let client_factory = minibot_irc::client::ClientFactory::create()?;
        let mut client = client_factory
            .connect("irc.chat.twitch.tv", 6697, "ludofex", &key)
            .await?;
        client.join("ludofex").await?;
        client.close().await?;
    }
    Ok(())
}
