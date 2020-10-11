use crate::read_bytes::ReadBytes;
use crate::write_bytes::WriteBytes;
use crate::Message;
use bytes::buf::Buf;

const MSG_TERM: &[u8] = b"\r\n";

#[derive(Clone, Debug)]
pub struct IrcCodec;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Message(#[from] crate::messages::Error),
}

impl From<std::convert::Infallible> for Error {
    fn from(f: std::convert::Infallible) -> Error {
        match f {}
    }
}

impl futures_codec::Decoder for IrcCodec {
    type Item = Message;
    type Error = Error;

    fn decode(
        &mut self,
        data: &mut futures_codec::BytesMut,
    ) -> Result<Option<Self::Item>, Self::Error> {
        // Try to find a terminator for the next message.
        loop {
            match data.windows(MSG_TERM.len()).position(|w| w == MSG_TERM) {
                None => break Ok(None),
                Some(pos) => {
                    // Read the message contents up to here
                    let message_contents = data.split_to(pos);
                    data.advance(MSG_TERM.len());

                    // Empty messages can just be skipped.
                    if pos > 0 {
                        break Ok(Some(Message::read_bytes(&message_contents)?));
                    }
                }
            }
        }
    }
}

impl futures_codec::Encoder for IrcCodec {
    type Item = crate::Message;
    type Error = Error;

    fn encode(
        &mut self,
        msg: crate::Message,
        out: &mut futures_codec::BytesMut,
    ) -> Result<(), Self::Error> {
        let mut msg_bytes: Vec<u8> = Vec::new();
        msg.write_bytes(&mut msg_bytes)?;
        out.reserve(msg_bytes.len() + MSG_TERM.len());
        out.extend_from_slice(&msg_bytes);
        out.extend_from_slice(&MSG_TERM);
        Ok(())
    }
}
