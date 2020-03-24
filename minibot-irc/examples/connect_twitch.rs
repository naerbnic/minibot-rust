extern crate tokio;
extern crate minibot_irc;
extern crate anyhow;
extern crate futures;

use futures::prelude::*;
use minibot_irc::messages::{Message, Command};

devsecrets::import_id!(DEVSECRETS_ID);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let ds = devsecrets::DevSecrets::from_id(&DEVSECRETS_ID).unwrap().unwrap();
    let key = ds.read_from("irc_key.txt").to_string().unwrap();

    let pass_message = minibot_irc::messages::Message::from_command_params(Command::from_name("PASS"), &[&*key]);
    let nick_message = Message::from_command_params(Command::from_name("NICK"), &["ludofex"]);

    let connector = minibot_irc::connection::make_connector()?;
    let (read, mut write) = minibot_irc::connection::irc_connect_ssl(&connector, "irc.chat.twitch.tv", 6697).await?;
    println!("Connected and streams created.");
    assert!(write.send(pass_message).await.is_ok());
    assert!(write.send(nick_message).await.is_ok());
    println!("PASS sent.");
    read.try_for_each(|msg| async move { println!("Server Msg: {:?}", msg); Ok(()) }).await?;
    Ok(())
}
