use crate::read_bytes::ReadBytes;
use crate::write_bytes::{ByteSink, WriteBytes};
use bytes::{Buf as _, BytesMut};

#[derive(Clone)]
pub struct IrcCodec;

impl tokio_util::codec::Decoder for IrcCodec {
    type Item = crate::messages::Message;

    type Error = super::Error;

    fn decode(&mut self, src: &mut BytesMut) -> super::Result<Option<Self::Item>> {
        // Look for ending CR LF
        let mut src_bytes;
        let pos = loop {
            src_bytes = src.bytes();
            let pos = match src_bytes.windows(2).position(|s| s == b"\r\n") {
                None => return Ok(None),
                Some(p) => p,
            };

            if pos == 0 {
                // An empty message is just skipped
                src.advance(2);
            } else {
                break pos;
            }
        };

        let message = crate::messages::Message::read_bytes(&src_bytes[..pos])?;
        src.advance(pos + 2);
        Ok(Some(message))
    }
}

impl tokio_util::codec::Encoder<crate::messages::Message> for IrcCodec {
    type Error = super::Error;

    fn encode(&mut self, item: crate::messages::Message, dst: &mut BytesMut) -> super::Result<()> {
        let mut result = Vec::new();
        item.write_bytes(&mut result).unwrap();
        item.write_bytes(dst).unwrap();
        dst.write(b"\r\n").unwrap();
        Ok(())
    }
}
