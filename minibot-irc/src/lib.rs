pub mod byte_string;
pub mod client;
pub mod connection;
mod futures_util;
pub mod messages;
pub mod read_bytes;
pub mod room_state;
pub mod rpc;
pub mod write_bytes;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
