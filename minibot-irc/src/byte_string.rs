use std::{fmt, ops};

pub struct ByteString(Vec<u8>);

impl ops::Deref for ByteString {
    type Target = ByteStr;
    fn deref(&self) -> &ByteStr {
        unsafe { std::mem::transmute(&self.0[..]) }
    }
}

impl ops::DerefMut for ByteString {
    fn deref_mut(&mut self) -> &mut ByteStr {
        unsafe { std::mem::transmute(&mut self.0[..]) }
    }
}

impl AsRef<[u8]> for ByteString {
    fn as_ref(&self) -> &[u8] {
        &self.0[..]
    }
}

impl AsMut<[u8]> for ByteString {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0[..]
    }
}

impl fmt::Debug for ByteString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

pub struct ByteStr([u8]);

impl AsRef<[u8]> for ByteStr {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl AsMut<[u8]> for ByteStr {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

impl fmt::Debug for ByteStr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("b\"")?;
        let mut curr_slice = &self.0;
        while !curr_slice.is_empty() {
            let (valid_str, rest): (&str, &[u8]) = match std::str::from_utf8(curr_slice) {
                Ok(s) => (s, &[]),
                Err(e) => (
                    std::str::from_utf8(&curr_slice[..e.valid_up_to()]).unwrap(),
                    &curr_slice[e.valid_up_to()..],
                ),
            };

            let mut buffer = [0u8; 4];

            for ch in valid_str.chars() {
                let encoded_ch = match ch {
                    '\n' => "\\n",
                    '\r' => "\\r",
                    '\t' => "\\t",
                    '\"' => "\\\"",
                    _ => ch.encode_utf8(&mut buffer),
                };
                f.write_str(encoded_ch)?;
            }

            curr_slice = match rest.split_first() {
                None => &[],
                Some((b, rest)) => {
                    f.write_fmt(format_args!("\\x{:2x}", b))?;
                    rest
                }
            };
        }
        f.write_str("\"")
    }
}
