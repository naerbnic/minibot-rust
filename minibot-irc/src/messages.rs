use std::collections::HashMap;

macro_rules! ensure {
    ($e:expr, $($fmt:expr),+) => {
        if $e {
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
                    _ => bail!("Unexpected char in tag value escape: {:?}", ch),
                }),
            },
            Some(ch) => result.push(ch),
        }
    }
    Ok(result)
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

struct Command(String);

impl Command {
    pub fn parse(text: &[u8]) -> Result<Self> {
        ensure!(!text.is_empty(), "Command must not be empty");
        Ok(Command(std::str::from_utf8(text)?.to_string()))
    }
}

struct Source {
    nick: Option<String>,
    user: Option<String>,
    host: Option<Vec<u8>>,
}

impl Source {
    pub fn parse(text: &[u8]) -> Result<Self> {
        let bang_index = text.iter().position(|c| c == &b'!');
        let at_index = text.iter().position(|c| c == &b'@');
        let (nick, user, host): (&[u8], &[u8], &[u8]) = match (bang_index, at_index) {
            (None, None) => (&[], &[], text),
            (Some(bang_index), None) => (&text[..bang_index], &text[bang_index + 1..], &[]),
            (None, Some(at_index)) => (&text[..at_index], &[], &text[at_index + 1..]),
            (Some(bang_index), Some(at_index)) => {
                ensure!(
                    bang_index < at_index,
                    "! must come before @ in source. Source: {:?}",
                    text
                );
                (
                    &text[..bang_index],
                    &text[bang_index + 1..at_index],
                    &text[at_index + 1..],
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

pub struct Message {
    tags: HashMap<String, String>,
    source: Option<Source>,
    command: Command,
    params: Vec<Vec<u8>>,
}

impl Message {
    fn from_line(text: &[u8]) -> Result<Self> {
        ensure!(!text.is_empty(), "");
        fn eat_space(text: &mut &[u8]) {
            for (i, ch) in text.iter().copied().enumerate() {
                if ch != b' ' {
                    *text = &text[i..];
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

        let mut remaining_text = text;
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
            Some(Source::parse(source_word)?)
        } else {
            None
        };

        let command_word = until_space(&mut remaining_text);
        let command = Command::parse(command_word)?;

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
