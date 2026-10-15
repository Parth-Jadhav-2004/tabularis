const MAX_SQL_BYTES: usize = 262_144;
const MAX_TOKENS: usize = 32_768;

#[derive(Clone, Debug)]
pub(super) struct Identifier {
    pub name: String,
    pub quoted: bool,
}

#[derive(Debug)]
pub(super) enum Token {
    Identifier(Identifier),
    Dot,
    Open,
    Close,
    Comma,
    Semicolon,
    Other,
}

impl Token {
    pub(super) fn keyword(&self, keyword: &str) -> bool {
        matches!(self, Self::Identifier(identifier) if !identifier.quoted && identifier.name.eq_ignore_ascii_case(keyword))
    }
}

pub(super) fn tokenize(sql: &str) -> (Vec<Token>, bool) {
    let mut end = sql.len().min(MAX_SQL_BYTES);
    while !sql.is_char_boundary(end) {
        end -= 1;
    }
    let chars = sql[..end].chars().collect::<Vec<_>>();
    let mut index = 0;
    let mut tokens = Vec::new();
    while index < chars.len() && tokens.len() < MAX_TOKENS {
        let current = chars[index];
        let next = chars.get(index + 1).copied();
        if current.is_whitespace() {
            index += 1;
            continue;
        }
        if (current == '-' && next == Some('-')) || (current == '#' && next != Some('>')) {
            while index < chars.len() && chars[index] != '\n' {
                index += 1;
            }
            continue;
        }
        if current == '/' && next == Some('*') {
            index += 2;
            let mut depth = 1;
            while index < chars.len() && depth > 0 {
                if chars[index] == '/' && chars.get(index + 1) == Some(&'*') {
                    depth += 1;
                    index += 2;
                } else if chars[index] == '*' && chars.get(index + 1) == Some(&'/') {
                    depth -= 1;
                    index += 2;
                } else {
                    index += 1;
                }
            }
            continue;
        }
        if current == '$' {
            let mut tag_end = index + 1;
            while chars
                .get(tag_end)
                .is_some_and(|character| character.is_ascii_alphanumeric() || *character == '_')
            {
                tag_end += 1;
            }
            if chars.get(tag_end) == Some(&'$') {
                let tag = &chars[index..=tag_end];
                index = tag_end + 1;
                while index < chars.len() && !chars[index..].starts_with(tag) {
                    index += 1;
                }
                index = (index + tag.len()).min(chars.len());
                tokens.push(Token::Other);
                continue;
            }
        }
        if matches!(current, '\'' | '"' | '`' | '[') {
            let closing = if current == '[' { ']' } else { current };
            let mut name = String::new();
            index += 1;
            let mut closed = false;
            while index < chars.len() {
                let character = chars[index];
                index += 1;
                if character == closing {
                    if chars.get(index) == Some(&closing) {
                        name.push(closing);
                        index += 1;
                    } else {
                        closed = true;
                        break;
                    }
                } else if character == '\\' && current == '\'' {
                    // Accept the escaped-literal interpretation conservatively.
                    index = (index + 1).min(chars.len());
                } else {
                    name.push(character);
                }
            }
            tokens.push(if current == '\'' || !closed {
                Token::Other
            } else {
                Token::Identifier(Identifier { name, quoted: true })
            });
            continue;
        }
        if current.is_alphabetic() || matches!(current, '_' | '$') {
            let start = index;
            index += 1;
            while chars.get(index).is_some_and(|character| {
                character.is_alphanumeric() || matches!(*character, '_' | '$')
            }) {
                index += 1;
            }
            tokens.push(Token::Identifier(Identifier {
                name: chars[start..index].iter().collect(),
                quoted: false,
            }));
            continue;
        }
        tokens.push(match current {
            '.' => Token::Dot,
            '(' => Token::Open,
            ')' => Token::Close,
            ',' => Token::Comma,
            ';' => Token::Semicolon,
            _ => Token::Other,
        });
        index += 1;
    }
    (tokens, end < sql.len() || index < chars.len())
}
