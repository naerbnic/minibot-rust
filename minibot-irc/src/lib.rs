pub mod client;
pub mod connection;
pub mod messages;
pub mod read_bytes;
pub mod room_state;
pub mod rpc;
pub mod write_bytes;
mod futures_util;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
