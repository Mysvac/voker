//! Implement `AccessPath` for &str.

use alloc::borrow::Cow;
use alloc::format;
use core::fmt::{self, Write};

use crate::access::{AccessPath, Accessor, OffsetAccessor, ParseError};

// -----------------------------------------------------------------------------
// Ident

struct Ident<'a>(&'a str);

impl<'a> Ident<'a> {
    /// A string that correctly represents a positive integer will be converted to [`Accessor::TupleIndex`].
    /// All other string will be converted to [`Accessor::FieldName`] (Including incorrect ident, such as "1a2").
    ///
    /// Parses an identifier after `.`:
    /// - A positive integer becomes [`Accessor::TupleIndex`].
    /// - Anything else becomes [`Accessor::FieldName`] (including invalid idents like `1a2`).
    ///
    /// Both forms begin with `.` and cannot be distinguished without parsing the ident.
    #[inline(always)]
    fn field(self) -> Accessor<'a> {
        match self.0.parse() {
            Ok(index) => Accessor::TupleIndex(index),
            Err(_) => Accessor::FieldName(self.0.into()),
        }
    }

    #[inline(always)]
    fn field_index(self) -> Result<Accessor<'a>, InnerError<'a>> {
        match self.0.parse() {
            Ok(index) => Ok(Accessor::FieldIndex(index)),
            Err(_) => Err(InnerError::InvalidIndex(self)),
        }
    }

    #[inline(always)]
    fn list_index(self) -> Result<Accessor<'a>, InnerError<'a>> {
        match self.0.parse() {
            Ok(index) => Ok(Accessor::ListIndex(index)),
            Err(_) => Err(InnerError::InvalidIndex(self)),
        }
    }
}

// -----------------------------------------------------------------------------
// Token

// NOTE: We use repr(u8) so that the `match byte` in `Token::symbol_from_byte`
// becomes a "check `byte` is one of SYMBOLS and forward its value" this makes
// the optimizer happy, and shaves off a few cycles.
#[repr(u8)]
enum Token<'a> {
    Dot = b'.',
    Pound = b'#',
    OpenBracket = b'[',
    CloseBracket = b']',
    Ident(Ident<'a>),
}

impl Token<'_> {
    const SYMBOLS: &'static [u8] = b".#[]";

    #[inline]
    fn symbol_from_byte(byte: u8) -> Option<Self> {
        match byte {
            b'.' => Some(Self::Dot),
            b'#' => Some(Self::Pound),
            b'[' => Some(Self::OpenBracket),
            b']' => Some(Self::CloseBracket),
            _ => None,
        }
    }
}

impl fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Dot => f.write_char('.'),
            Token::Pound => f.write_char('#'),
            Token::OpenBracket => f.write_char('['),
            Token::CloseBracket => f.write_char(']'),
            Token::Ident(ident) => f.write_str(ident.0),
        }
    }
}

// -----------------------------------------------------------------------------
// Error

// Seal visibility
enum InnerError<'a> {
    NoIdent,
    IsNotIdent(Token<'a>),
    UnexpectedIdent(Ident<'a>),
    InvalidIndex(Ident<'a>),
    Unclosed,
    BadClose(Token<'a>),
    CloseBeforeOpen,
}

impl<'a> InnerError<'a> {
    fn as_cow(&self) -> Cow<'a, str> {
        match self {
            InnerError::NoIdent => "expected an identifier, but reached end of path string".into(),
            InnerError::IsNotIdent(token) => {
                format!("expected an identifier, got '{token}' instead").into()
            }
            InnerError::UnexpectedIdent(ident) => {
                format!("expected a keyword ('#.[]'), got '{}' instead", ident.0).into()
            }
            InnerError::InvalidIndex(ident) => {
                format!("failed to parse index as integer: {}", ident.0).into()
            }
            InnerError::Unclosed => {
                "a '[' wasn't closed, reached end of path string before finding a ']'".into()
            }
            InnerError::BadClose(token) => {
                format!("a '[' wasn't closed properly, got '{token}' instead").into()
            }
            InnerError::CloseBeforeOpen => "a ']' was found before an opening '['".into(),
        }
    }
}

// -----------------------------------------------------------------------------
// PathParser

// A one-time path parser
struct PathParser<'a> {
    path: &'a str,
    remaining: &'a [u8],
}

impl<'a> PathParser<'a> {
    // Get the next token, skip spaces.
    //
    // The obtained ident will remove the trailing space.
    fn next_token(&mut self) -> Option<Token<'a>> {
        let to_parse = self.remaining.trim_ascii_start();

        let (first_byte, remaining) = to_parse.split_first()?;

        if let Some(token) = Token::symbol_from_byte(*first_byte) {
            self.remaining = remaining;
            return Some(token);
        }

        // find indent
        let ident_len = to_parse.iter().position(|t| Token::SYMBOLS.contains(t));
        let (ident, remaining) = to_parse.split_at(ident_len.unwrap_or(to_parse.len()));
        let ident = ident.trim_ascii_end();

        // # Safety
        // - `&str` should be a valid UTF-8 string.
        // - Ensure that the passed bytes are valid UTF-8.
        #[expect(unsafe_code, reason = "Ensure that the passed bytes are valid UTF-8.")]
        let ident = unsafe { core::str::from_utf8_unchecked(ident) };

        self.remaining = remaining;
        Some(Token::Ident(Ident(ident)))
    }

    #[inline]
    fn next_ident(&mut self) -> Result<Ident<'a>, InnerError<'a>> {
        match self.next_token() {
            Some(Token::Ident(ident)) => Ok(ident),
            Some(other) => Err(InnerError::IsNotIdent(other)),
            None => Err(InnerError::NoIdent),
        }
    }

    fn following_accessor(&mut self, token: Token<'a>) -> Result<Accessor<'a>, InnerError<'a>> {
        match token {
            Token::Dot => Ok(self.next_ident()?.field()),
            Token::Pound => self.next_ident()?.field_index(),
            Token::OpenBracket => {
                let index_ident = self.next_ident()?.list_index()?;
                match self.next_token() {
                    Some(Token::CloseBracket) => Ok(index_ident),
                    Some(other) => Err(InnerError::BadClose(other)),
                    None => Err(InnerError::Unclosed),
                }
            }
            Token::CloseBracket => Err(InnerError::CloseBeforeOpen),
            Token::Ident(ident) => Err(InnerError::UnexpectedIdent(ident)),
        }
    }
}

impl<'a> Iterator for PathParser<'a> {
    type Item = Result<OffsetAccessor<'a>, ParseError<'a>>;

    #[inline(never)]
    fn next(&mut self) -> Option<Self::Item> {
        let token = self.next_token()?;
        let offset = self.path.len() - self.remaining.len();

        let res = match self.following_accessor(token) {
            Ok(accessor) => Ok(OffsetAccessor {
                accessor,
                offset: Some(offset),
            }),
            Err(err) => {
                // Ensure that next returns `None` after an error occurs
                self.remaining = "".as_bytes();
                Err(ParseError {
                    offset,
                    path: self.path,
                    error: err.as_cow(),
                })
            }
        };

        Some(res)
    }
}

// -----------------------------------------------------------------------------
// Impl for &str

// impl for str
impl<'a> AccessPath<'a> for &'a str {
    #[inline]
    fn parse_to_accessor(
        &self,
    ) -> impl Iterator<Item = Result<OffsetAccessor<'a>, ParseError<'a>>> {
        PathParser {
            path: self,
            remaining: self.as_bytes(),
        }
    }
}
