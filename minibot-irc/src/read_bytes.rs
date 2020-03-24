

pub trait ReadBytes: Sized{
    type Err;
    fn read_bytes(buf: &[u8]) -> Result<Self, Self::Err>;
}
