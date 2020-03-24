pub trait ByteSink {
    type Err;
    fn write(&mut self, bytes: &[u8]) -> Result<(), Self::Err>;
}

impl ByteSink for Vec<u8> {
    type Err = std::convert::Infallible;
    fn write(&mut self, bytes: &[u8]) -> Result<(), Self::Err> {
        self.extend_from_slice(bytes);
        Ok(())
    }
}

impl ByteSink for bytes::BytesMut {
    type Err = std::convert::Infallible;
    fn write(&mut self, bytes: &[u8]) -> Result<(), Self::Err> {
        self.extend_from_slice(bytes);
        Ok(())
    }
}

pub trait WriteBytes {
    fn write_bytes<T: ByteSink>(&self, out: &mut T) -> Result<(), T::Err>;
}
