use std::{fmt, ops};

pub struct ByteString(Vec<u8>);

impl ByteString {
    pub fn from(data: Vec<u8>) -> Self {
        ByteString(data)
    }

    pub fn from_slice(data: &[u8]) -> Self {
        ByteString(Vec::from(data))
    }
}

impl ops::Deref for ByteString {
    type Target = ByteStr;
    fn deref(&self) -> &ByteStr {
        ByteStr::from(&self.0[..])
    }
}

impl ops::DerefMut for ByteString {
    fn deref_mut(&mut self) -> &mut ByteStr {
        ByteStr::from_mut(&mut self.0[..])
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

impl std::iter::FromIterator<u8> for ByteString {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = u8>,
    {
        ByteString(iter.into_iter().collect())
    }
}

pub struct ByteStr([u8]);

impl ByteStr {
    pub fn from(data: &[u8]) -> &Self {
        return unsafe { std::mem::transmute(data) };
    }

    pub fn from_mut(data: &mut [u8]) -> &mut Self {
        return unsafe { std::mem::transmute(data) };
    }

    pub fn eq_bytes(&self, s: &[u8]) -> bool {
        &self.0 == s
    }

    pub fn split_spaces(&self) -> SplitSpaces {
        SplitSpaces(&self.0)
    }

    pub fn to_byte_string(&self) -> ByteString {
        ByteString::from_slice(&self.0)
    }
}

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

pub struct SplitSpaces<'a>(&'a [u8]);

impl<'a> Iterator for SplitSpaces<'a> {
    type Item = &'a ByteStr;
    fn next(&mut self) -> Option<&'a ByteStr> {
        self.0 = match self.0.iter().position(|b| b != &b' ') {
            None => &[],
            Some(i) => &self.0[i..],
        };

        if self.0.is_empty() {
            None
        } else {
            let (next, rest): (&[u8], &[u8]) = match self.0.iter().position(|b| b == &b' ') {
                None => (self.0, &[]),
                Some(i) => (&self.0[..i], &self.0[i..]),
            };

            self.0 = rest;
            Some(ByteStr::from(next))
        }
    }
}
