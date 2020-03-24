use super::read_bytes::ReadBytes;
use super::write_bytes::{ByteSink, WriteBytes};
use std::borrow::Cow;
use std::collections::HashMap;

macro_rules! ensure {
    ($e:expr, $($fmt:expr),+) => {
        if !$e {
            return Err(Error::Text(std::format!($($fmt),*).into()));
        }
    };
}

macro_rules! bail {
    ($($fmt:expr),+) => {
        return Err(Error::Text(std::format!($($fmt),*).into()));
    }
}

#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("Error: {0}")]
    Text(String),

    #[error("UTF8 codec error: {0:?}")]
    Utf8Error(#[from] std::str::Utf8Error),
}

type Result<T> = std::result::Result<T, Error>;

fn unescape_tag_value(val: &[u8]) -> Result<String> {
    let value_chars = std::str::from_utf8(val)?;
    let mut result = String::new();
    let mut char_iter = value_chars.chars();
    loop {
        match char_iter.next() {
            None => break,
            Some(ch) if ch == '\\' => match char_iter.next() {
                None => break,
                Some(ch) => result.push(match ch {
                    '\\' => '\\',
                    ':' => ';',
                    'r' => '\r',
                    'n' => '\n',
                    's' => ' ',
                    _ => bail!("Unexpected char in tag value escape: {:?}", ch),
                }),
            },
            Some(ch) => result.push(ch),
        }
    }
    Ok(result)
}

fn escape_tag_value<T: ByteSink>(val: &str, out: &mut T) -> std::result::Result<(), T::Err> {
    for b in val.as_bytes() {
        let escaped_b = match b {
            b'\\' => b"\\\\",
            b';' => b"\\:",
            b'\r' => b"\\r",
            b'\n' => b"\\n",
            b' ' => b"\\s",
            b => std::slice::from_ref(b),
        };
        out.write(escaped_b)?;
    }
    Ok(())
}

fn parse_tags(tag_word: &[u8]) -> Result<HashMap<String, String>> {
    let mut result = HashMap::new();
    for term in tag_word.split(|c| c == &b';') {
        let (key_bytes, value_bytes): (&[u8], &[u8]) = match term.iter().position(|c| c == &b'=') {
            None => (term, &[]),
            Some(p) => (&term[..p], &term[p + 1..]),
        };

        result.insert(
            std::str::from_utf8(key_bytes)?.to_string(),
            unescape_tag_value(value_bytes)?,
        );
    }
    Ok(result)
}

#[derive(Debug)]
pub struct Command(String);

impl Command {
    pub fn from_name<'a>(name: impl Into<Cow<'a, str>>) -> Self {
        let name = name.into().into_owned();
        for ch in name.chars() {
            assert!(ch.is_ascii());
            assert!(ch.is_alphabetic());
        }
        Command(name)
    }

    pub fn from_numeric(n: u16) -> Self {
        assert!(n < 1000);
        Command(format!("{:03}", n))
    }
}

impl ReadBytes for Command {
    type Err = Error;
    fn read_bytes(buf: &[u8]) -> Result<Self> {
        ensure!(!buf.is_empty(), "Command must not be empty");
        Ok(Command(std::str::from_utf8(buf)?.to_string()))
    }
}

impl WriteBytes for Command {
    fn write_bytes<T: ByteSink>(&self, out: &mut T) -> std::result::Result<(), T::Err> {
        out.write(self.0.as_bytes())
    }
}

#[derive(Debug)]
pub struct Source {
    nick: Option<String>,
    user: Option<String>,
    host: Option<Vec<u8>>,
}

impl ReadBytes for Source {
    type Err = Error;
    fn read_bytes(buf: &[u8]) -> Result<Self> {
        let bang_index = buf.iter().position(|c| c == &b'!');
        let at_index = buf.iter().position(|c| c == &b'@');
        let (nick, user, host): (&[u8], &[u8], &[u8]) = match (bang_index, at_index) {
            (None, None) => (&[], &[], buf),
            (Some(bang_index), None) => (&buf[..bang_index], &buf[bang_index + 1..], &[]),
            (None, Some(at_index)) => (&buf[..at_index], &[], &buf[at_index + 1..]),
            (Some(bang_index), Some(at_index)) => {
                ensure!(
                    bang_index < at_index,
                    "! must come before @ in source. Source: {:?}",
                    buf
                );
                (
                    &buf[..bang_index],
                    &buf[bang_index + 1..at_index],
                    &buf[at_index + 1..],
                )
            }
        };

        fn none_if_empty<T: AsRef<[u8]>>(opt_text: T) -> Option<T> {
            if opt_text.as_ref().is_empty() {
                None
            } else {
                Some(opt_text)
            }
        }

        let nick = none_if_empty(std::str::from_utf8(nick)?.to_string());
        let user = none_if_empty(std::str::from_utf8(user)?.to_string());
        let host = none_if_empty(host.iter().copied().collect());

        Ok(Source { nick, user, host })
    }
}

impl WriteBytes for Source {
    fn write_bytes<T: ByteSink>(&self, out: &mut T) -> std::result::Result<(), T::Err> {
        match (&self.nick, &self.user, &self.host) {
            (None, None, Some(host)) => out.write(host)?,
            (Some(nick), None, None) => {
                out.write(nick.as_bytes())?;
                out.write(b"!")?;
            }
            (nick, Some(user), None) => {
                out.write(nick.as_ref().map_or("", |s| s.as_str()).as_bytes())?;
                out.write(b"!")?;
                out.write(user.as_bytes())?;
            }
            (nick, user, Some(host)) => {
                out.write(nick.as_ref().map_or("", String::as_str).as_bytes())?;
                out.write(b"!")?;
                out.write(user.as_ref().map_or("", String::as_str).as_bytes())?;
                out.write(b"@")?;
                out.write(host)?;
            }
            (None, None, None) => unreachable!("Prevented by construction."),
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Message {
    tags: HashMap<String, String>,
    source: Option<Source>,
    command: Command,
    params: Vec<Vec<u8>>,
}

impl Message {
    pub fn from_command_params<T: AsRef<[S]>, S: AsRef<[u8]>>(cmd: Command, params: T) -> Self {
        let params = params.as_ref().iter().map(|p| p.as_ref().to_vec()).collect::<Vec<_>>();
        Message {
            tags: HashMap::new(),
            source: None,
            command: cmd,
            params,
        }
    }
}

impl ReadBytes for Message {
    type Err = Error;
    fn read_bytes(buf: &[u8]) -> Result<Self> {
        ensure!(!buf.is_empty(), "Message must not be empty.");
        fn eat_space(text: &mut &[u8]) {
            for (i, ch) in text.iter().copied().enumerate() {
                if ch != b' ' {
                    *text = &text[i..];
                    return;
                }
            }
            *text = &[];
        }
        fn until_space<'a>(text: &mut &'a [u8]) -> &'a [u8] {
            for (i, ch) in text.iter().copied().enumerate() {
                if ch == b' ' {
                    let word_slice = &text[..i];
                    *text = &text[i..];
                    eat_space(text);
                    return word_slice;
                }
            }

            let word_slice = &text[..];
            *text = &[];
            word_slice
        }

        fn get_first_char(text: &[u8]) -> Option<u8> {
            match text.iter().next() {
                Some(ch) => Some(*ch),
                None => None,
            }
        }


        let mut remaining_text = buf;
        let first_char = get_first_char(remaining_text);
        let tags = if first_char == Some(b'@') {
            let tags_word = until_space(&mut remaining_text);
            ensure!(!remaining_text.is_empty(), "Did not find IRC command");
            parse_tags(tags_word)?
        } else {
            HashMap::new()
        };

        let first_char = get_first_char(remaining_text);
        let source = if first_char == Some(b':') {
            let source_word = until_space(&mut remaining_text);
            ensure!(!remaining_text.is_empty(), "Did not find IRC command");
            Some(Source::read_bytes(source_word)?)
        } else {
            None
        };

        let command_word = until_space(&mut remaining_text);
        let command = Command::read_bytes(command_word)?;

        let mut params = Vec::new();

        while !remaining_text.is_empty() {
            if get_first_char(remaining_text) == Some(b':') {
                params.push(remaining_text[1..].to_owned());
                remaining_text = &[];
            } else {
                let param_word = until_space(&mut remaining_text);
                params.push(param_word.to_owned());
            }
        }

        Ok(Message {
            tags,
            source,
            command,
            params,
        })
    }
}

impl WriteBytes for Message {
    fn write_bytes<T: ByteSink>(&self, out: &mut T) -> std::result::Result<(), T::Err> {
        if !self.tags.is_empty() {
            out.write(b"@")?;
            let mut first_tag = true;
            for (k, v) in self.tags.iter() {
                if first_tag {
                    first_tag = false;
                } else {
                    out.write(b";")?;
                }

                assert!(!k.is_empty());
                out.write(k.as_bytes())?;
                if !v.is_empty() {
                    out.write(b"=")?;
                    escape_tag_value(&v, out)?;
                }
            }

            out.write(b" ")?;
        }

        if let Some(source) = &self.source {
            out.write(b":")?;
            source.write_bytes(out)?;
            out.write(b" ")?;
        }

        self.command.write_bytes(out)?;
        out.write(b" ")?;

        if let Some((last, rest)) = self.params.split_last() {
            for param in rest {
                out.write(param)?;
                out.write(b" ")?;
            }

            out.write(b":")?;
            out.write(last)?;
        }
        Ok(())
    }
}
