pub trait ByteSink {
    type Err;
    fn write<T: AsRef<[u8]>>(&mut self, bytes: T) -> Result<(), Self::Err>;
}

impl ByteSink for Vec<u8> {
    type Err = std::convert::Infallible;
    fn write<T: AsRef<[u8]>>(&mut self, bytes: T) -> Result<(), Self::Err> {
        self.extend_from_slice(bytes.as_ref());
        Ok(())
    }
}

impl ByteSink for bytes::BytesMut {
    type Err = std::convert::Infallible;
    fn write<T: AsRef<[u8]>>(&mut self, bytes: T) -> Result<(), Self::Err> {
        self.extend_from_slice(bytes.as_ref());
        Ok(())
    }
}

pub trait WriteBytes {
    fn write_bytes<T: ByteSink>(&self, out: &mut T) -> Result<(), T::Err>;
}
