use std::collections::HashMap;

fn parse_tags(tag_word: &str) -> HashMap<String, String> {
    todo!()
}

struct Command(String);

impl Command {
    pub fn parse(text: &str) -> anyhow::Result<Self> {
        anyhow::ensure!(!text.is_empty(), "Command must not be empty");
        Ok(Command(text.to_string()))
    }
}

struct Source {
    nick: Option<String>,
    user: Option<String>,
    host: Option<String>,
}

impl Source {
    pub fn parse(text: &str) -> anyhow::Result<Self> {
        let bang_index = text.find('!');
        let at_index = text.find('@');
        let mut source = match (bang_index, at_index) {
            (None, None) => Source {
                nick: None,
                user: None,
                host: Some(text.to_string()),
            },
            (Some(bang_index), None) => Source {
                nick: Some(text[..bang_index].to_string()),
                user: Some(text[bang_index + 1..].to_string()),
                host: None,
            },
            (None, Some(at_index)) => Source {
                nick: Some(text[..at_index].to_string()),
                user: None,
                host: Some(text[at_index + 1..].to_string()),
            },
            (Some(bang_index), Some(at_index)) => {
                anyhow::ensure!(
                    bang_index < at_index,
                    "! must come before @ in source. Source: {:?}",
                    text
                );
                Source {
                    nick: Some(text[..bang_index].to_string()),
                    user: Some(text[bang_index + 1..at_index].to_string()),
                    host: Some(text[at_index + 1..].to_string()),
                }
            }
        };

        fn none_if_empty(opt_text: &mut Option<String>) {
            if let Some(text) = opt_text {
                if text.is_empty() {
                    *opt_text = None
                }
            }
        }

        none_if_empty(&mut source.nick);
        none_if_empty(&mut source.user);
        none_if_empty(&mut source.host);

        Ok(source)
    }
}

pub struct Message {
    tags: HashMap<String, String>,
    source: Option<Source>,
    command: Command,
    params: Vec<String>,
}

impl Message {
    fn from_line(text: &str) -> anyhow::Result<Self> {
        anyhow::ensure!(!text.is_empty(), "");
        fn eat_space(text: &mut &str) {
            for (i, ch) in text.char_indices() {
                if ch != ' ' {
                    *text = &text[i..];
                }
            }
            *text = "";
        }
        fn until_space<'a>(text: &mut &'a str) -> &'a str {
            for (i, ch) in text.char_indices() {
                if ch == ' ' {
                    let word_slice = &text[..i];
                    *text = &text[i..];
                    eat_space(text);
                    return word_slice;
                }
            }

            let word_slice = &text[..];
            *text = "";
            word_slice
        }

        fn get_first_char(text: &str) -> Option<char> {
            match text.chars().next() {
                Some(ch) => Some(ch),
                None => None,
            }
        }

        let mut remaining_text = text;
        let first_char = get_first_char(remaining_text);
        let tags = if first_char == Some('@') {
            let tags_word = until_space(&mut remaining_text);
            anyhow::ensure!(!remaining_text.is_empty(), "Did not find IRC command");
            parse_tags(tags_word)
        } else {
            HashMap::new()
        };

        let first_char = get_first_char(remaining_text);
        let source = if first_char == Some(':') {
            let source_word = until_space(&mut remaining_text);
            anyhow::ensure!(!remaining_text.is_empty(), "Did not find IRC command");
            Some(Source::parse(source_word)?)
        } else {
            None
        };

        let command_word = until_space(&mut remaining_text);
        let command = Command::parse(command_word)?;

        let mut params = Vec::new();

        while !remaining_text.is_empty() {
            if get_first_char(remaining_text) == Some(':') {
                params.push(remaining_text[1..].to_string());
                remaining_text = "";
            } else {
                let param_word = until_space(&mut remaining_text);
                params.push(param_word.to_string());
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
