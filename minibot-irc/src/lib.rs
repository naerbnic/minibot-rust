pub mod connection;
pub mod messages;
pub mod read_bytes;
pub mod rpc;
pub mod write_bytes;
pub mod client;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
