pub mod byte_string;
pub mod client;
pub mod connection;
mod futures_util;
pub mod room_state;
pub mod rpc;

pub use minibot_irc_raw::{Message, Command};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
