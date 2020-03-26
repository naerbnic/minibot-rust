use super::read_bytes::ReadBytes;
use super::write_bytes::{ByteSink, WriteBytes};
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;

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

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct CommandNumber(u16);

impl CommandNumber {
    pub fn new(i: u16) -> Self {
        assert!(
            i > 0 && i < 1000,
            "A command number must be between 1 and 999 inclusive. Got {:?}",
            i
        );
        CommandNumber(i)
    }

    pub fn number(&self) -> u16 {
        self.0
    }
}

impl From<CommandNumber> for u16 {
    fn from(n: CommandNumber) -> u16 {
        n.number()
    }
}

impl fmt::Debug for CommandNumber {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_fmt(format_args!("{:03}", self.0))
    }
}

pub enum Command {
    Name(String),
    Num(CommandNumber),
}

impl Command {
    pub fn from_name<'a>(name: impl Into<Cow<'a, str>>) -> Self {
        let name = name.into().into_owned();
        for ch in name.chars() {
            assert!(ch.is_ascii());
            assert!(ch.is_alphabetic());
        }
        Command::Name(name)
    }

    pub fn from_numeric(n: u16) -> Self {
        Command::Num(CommandNumber::new(n))
    }
}

impl fmt::Debug for Command {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Command::Name(n) => f.write_str(n),
            Command::Num(n) => fmt::Debug::fmt(n, f),
        }
    }
}

impl ReadBytes for Command {
    type Err = Error;
    fn read_bytes(buf: &[u8]) -> Result<Self> {
        ensure!(!buf.is_empty(), "Command must not be empty");
        if buf[0].is_ascii_digit() {
            ensure!(
                buf.len() == 3,
                "Numeric command must be exactly 3 characters long. Got {:?}",
                String::from_utf8_lossy(buf)
            );
            ensure!(
                buf.iter().all(u8::is_ascii_digit),
                "Numeric command must be all ascii numbers."
            );

            let mut total = 0u16;
            for &b in buf {
                total = total * 10 + (b - b'0') as u16;
            }

            Ok(Command::Num(CommandNumber::new(total)))
        } else {
            ensure!(
                buf.iter().all(u8::is_ascii_alphabetic),
                "Name command must be all ascii letters."
            );
            Ok(Command::Name(String::from_utf8(buf.to_vec()).unwrap()))
        }
    }
}

impl WriteBytes for Command {
    fn write_bytes<T: ByteSink>(&self, out: &mut T) -> std::result::Result<(), T::Err> {
        match self {
            Command::Name(n) => out.write(n.as_bytes()),
            Command::Num(n) => out.write(format!("{:03}", n.number()).as_bytes()),
        }
    }
}

pub struct Source {
    nick: Option<String>,
    user: Option<String>,
    host: Option<Vec<u8>>,
}

impl std::fmt::Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut source = f.debug_struct("Source");
        if let Some(nick) = &self.nick {
            source.field("nick", nick);
        }

        if let Some(user) = &self.nick {
            source.field("user", user);
        }

        if let Some(host) = &self.host {
            source.field("host", &String::from_utf8_lossy(host).as_ref());
        }

        source.finish()
    }
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

pub struct Message {
    tags: HashMap<String, String>,
    source: Option<Source>,
    command: Command,
    params: Vec<Vec<u8>>,
}

impl Message {
    pub fn from_named_command_params<T: AsRef<[S]>, S: AsRef<[u8]>>(cmd: &str, params: T) -> Self {
        Message::from_command_params(Command::from_name(cmd), params)
    }
    
    pub fn from_named_command<T: AsRef<[S]>, S: AsRef<[u8]>>(cmd: &str) -> Self {
        Message::from_command(Command::from_name(cmd))
    }

    pub fn from_command_params<T: AsRef<[S]>, S: AsRef<[u8]>>(cmd: Command, params: T) -> Self {
        let params = params
            .as_ref()
            .iter()
            .map(|p| p.as_ref().to_vec())
            .collect::<Vec<_>>();
        Message {
            tags: HashMap::new(),
            source: None,
            command: cmd,
            params,
        }
    }

    pub fn from_command(cmd: Command) -> Self {
        Message {
            tags: HashMap::new(),
            source: None,
            command: cmd,
            params: Vec::new(),
        }
    }

    pub fn has_named_command(&self, name: &str) -> bool {
        match &self.command {
            Command::Num(_) => false,
            Command::Name(n) => n == name,
        }
    }

    pub fn has_num_command(&self, num: u16) -> bool {
        match &self.command {
            Command::Num(n) => n.number() == num,
            Command::Name(_) => false,
        }
    }
}

impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut f = f.debug_struct("Message");

        if !self.tags.is_empty() {
            f.field("tags", &self.tags);
        }

        if let Some(source) = &self.source {
            f.field("source", source);
        }

        f.field("command", &self.command);

        if !self.params.is_empty() {
            let param_strs = self
                .params
                .iter()
                .map(|p| String::from_utf8_lossy(p))
                .collect::<Vec<_>>();
            f.field("params", &param_strs);
        }

        f.finish()
    }
}

impl ReadBytes for Message {
    type Err = Error;
    fn read_bytes(buf: &[u8]) -> Result<Self> {
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

        ensure!(!buf.is_empty(), "Message must not be empty.");
        let mut remaining_text = buf;
        let tags = if let Some((b'@', rest)) = remaining_text.split_first() {
            remaining_text = rest;
            let tags_word = until_space(&mut remaining_text);
            ensure!(!remaining_text.is_empty(), "Did not find IRC command");
            parse_tags(tags_word)?
        } else {
            HashMap::new()
        };

        let source = if let Some((b':', rest)) = remaining_text.split_first() {
            remaining_text = rest;
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
