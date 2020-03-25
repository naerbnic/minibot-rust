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

    let cap_ls_message = Message::from_command_params(Command::from_name("CAP"), &["LS", "302"]);
    let pass_message = Message::from_command_params(Command::from_name("PASS"), &[&key]);
    let nick_message = Message::from_command_params(Command::from_name("NICK"), &["ludofex"]);
    let cap_req_message = Message::from_command_params(Command::from_name("CAP"), &["REQ", "twitch.tv/tags twitch.tv/commands twitch.tv/membership"]);
    let cap_end_message = Message::from_command_params(Command::from_name("CAP"), &["END"]);
    let quit_message = Message::from_command(Command::from_name("QUIT"));

    let connector = minibot_irc::connection::make_connector()?;
    let (read, mut write) = minibot_irc::connection::irc_connect_ssl(&connector, "irc.chat.twitch.tv", 6697).await?;
    println!("Connected and streams created.");
    write.send(cap_ls_message).await?;
    write.send(cap_req_message).await?;
    write.send(pass_message).await?;
    write.send(nick_message).await?;
    write.send(cap_end_message).await?;
    write.send(quit_message).await?;
    println!("PASS sent.");
    read.try_for_each(|msg| async move { println!("Server Msg: {:?}", msg); Ok(()) }).await?;
    Ok(())
}
