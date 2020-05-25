pub mod vevent;
use std::io::BufRead;
use std::fmt::{self, Debug};

macro_rules! define_identifier_set {
    ( $Name:ident, $( $Tok:ident, $tok_name:expr ),* $(,)? ) => {
        #[derive(Debug, PartialEq, Eq, Copy, Clone)]
        pub enum $Name {
            $( $Tok, )*
        }
        impl $Name {
            fn from_bytes<'s>(s: &'s [u8]) -> std::result::Result<Self, &'s [u8]> {
                use $Name::*;
                Ok(match s {
                    $( $tok_name => $Tok, )*
                    _ => return Err(s)
                })
            }
            pub fn as_str(self) -> &'static str {
                use $Name::*;
                match self {
                    $( $Tok => unsafe { std::str::from_utf8_unchecked($tok_name) }, )*
                }
            }
        }
    };
}

define_identifier_set!(Property,
    // initial registry
    Calscale,        b"CALSCALE",
    Method,          b"METHOD",
    Prodid,          b"PRODID",
    Version,         b"VERSION",
    Attach,          b"ATTACH",
    Categories,      b"CATEGORIES",
    Class,           b"CLASS",
    Comment,         b"COMMENT",
    Description,     b"DESCRIPTION",
    Geo,             b"GEO",
    Location,        b"LOCATION",
    PercentComplete, b"PERCENT-COMPLETE",
    Priority,        b"PRIORITY",
    Resources,       b"RESOURCES",
    Status,          b"STATUS",
    Summary,         b"SUMMARY",
    Completed,       b"COMPLETED",
    Dtend,           b"DTEND",
    Due,             b"DUE",
    Dtstart,         b"DTSTART",
    Duration,        b"DURATION",
    Freebusy,        b"FREEBUSY",
    Transp,          b"TRANSP",
    Tzid,            b"TZID",
    Tzname,          b"TZNAME",
    Tzoffsetfrom,    b"TZOFFSETFROM",
    Tzoffsetto,      b"TZOFFSETTO",
    Tzurl,           b"TZURL",
    Attendee,        b"ATTENDEE",
    Contact,         b"CONTACT",
    Organizer,       b"ORGANIZER",
    RecurrenceId,    b"RECURRENCE-ID",
    RelatedTo,       b"RELATED-TO",
    Url,             b"URL",
    Uid,             b"UID",
    Exdate,          b"EXDATE",
    Exrule,          b"EXRULE",
    Rdate,           b"RDATE",
    Rrule,           b"RRULE",
    Action,          b"ACTION",
    Repeat,          b"REPEAT",
    Trigger,         b"TRIGGER",
    Created,         b"CREATED",
    Dtstamp,         b"DTSTAMP",
    LastModified,    b"LAST-MODIFIED",
    Sequence,        b"SEQUENCE",
    RequestStatus,   b"REQUEST-STATUS",
    // pseudo-properties
    Begin,           b"BEGIN",
    End,             b"END",
);

define_identifier_set!(ParamName,
    Altrep,        b"ALTREP",
    Cn,            b"CN",
    Cutype,        b"CUTYPE",
    DelegatedFrom, b"DELEGATED-FROM",
    DelegatedTo,   b"DELEGATED-TO",
    Dir,           b"DIR",
    Encoding,      b"ENCODING",
    Fmttype,       b"FMTTYPE",
    Fbtype,        b"FBTYPE",
    Language,      b"LANGUAGE",
    Member,        b"MEMBER",
    Partstat,      b"PARTSTAT",
    Range,         b"RANGE",
    Related,       b"RELATED",
    Reltype,       b"RELTYPE",
    Role,          b"ROLE",
    Rsvp,          b"RSVP",
    SentBy,        b"SENT-BY",
    Tzid,          b"TZID",
    Value,         b"VALUE",
);

#[derive(Debug)]
enum Condition {
    UnexpectedEof,
    Io(std::io::Error),
    Encoding(std::string::FromUtf8Error),
    BadProperty(Vec<u8>),
    BadParam(Vec<u8>),
}
pub struct Error {
    condition: Condition,
    line: usize,
}
impl Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "While parsing line {}: {:?}", self.line, self.condition)
    }
}
pub type Result<T> = std::result::Result<T, Error>;

pub struct Param {
    name: ParamName,
    values: Vec<String>,
}
impl Param {
    pub fn name(&self) -> ParamName {
        self.name
    }
    pub fn values(&self) -> impl Iterator<Item=&str> {
        self.values.iter().map(|s| s.as_str())
    }
}

pub struct ContentLine {
    name: Property,
    params: Vec<Param>,
    value: String,
    line: usize,
}
impl ContentLine {
    pub fn name(&self) -> Property {
        self.name
    }

    pub fn params(&self) -> impl Iterator<Item=&Param> {
        self.params.iter()
    }

    pub fn values_of(&self, pn: ParamName) -> Option<impl Iterator<Item=&str>> {
        for param in &self.params {
            if param.name() == pn {
                return Some(param.values.iter().map(|s| s.as_str()));
            }
        }
        None
    }

    pub fn value_of(&self, pn: ParamName) -> Option<&str> {
        for param in &self.params {
            if param.name() == pn {
                // XXX: what if len > 1?
                return param.values().next()
            }
        }
        None
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn line(&self) -> usize {
        self.line
    }
}

pub struct Lexer<S> {
    stream: S,
    line: usize,
}

fn bad_eof(line: usize) -> Error {
    Error { condition: Condition::UnexpectedEof, line }
}

fn bad_io(line: usize, e: std::io::Error) -> Error {
    Error { condition: Condition::Io(e), line }
}

fn bad_encoding(line: usize, e: std::string::FromUtf8Error) -> Error {
    Error { condition: Condition::Encoding(e), line }
}

fn bad_property(line: usize, s: &[u8]) -> Error {
    Error { condition: Condition::BadProperty(s.to_owned()), line }
}

fn bad_param(line: usize, s: &[u8]) -> Error {
    Error { condition: Condition::BadParam(s.to_owned()), line }
}

impl<S: BufRead> Lexer<S> {
    pub fn new(stream: S) -> Self {
        let line = 1;
        Self { stream, line }
    }

    pub fn lex_content_line(&mut self) -> Result<Option<ContentLine>> {
        let line = self.line;
        if self.stream.fill_buf().map_err(|e| bad_io(line, e))?.is_empty() {
            return Ok(None);
        }
        let name = Property::from_bytes(&self.read_identifier()?).map_err(|e| bad_property(line, e))?;
        let params = self.read_params()?;
        let value = self.read_value()?;
        Ok(Some(ContentLine { name, params, value, line }))
    }

    pub fn finish(self) -> S {
        self.stream
    }
}

impl<S: BufRead> Lexer<S> {
    /// Get the next octet, handling "unfolding" and normalization of raw line breaks from CRLF to LF.
    fn peek(&mut self) -> Result<u8> {
        Ok(loop {
            let line = self.line;
            let c = *self.stream.fill_buf().map_err(|e| bad_io(line, e))?
                .get(0).ok_or_else(|| bad_eof(line))?;
            match c {
                b'\r' => {
                    self.stream.consume(1);
                    let c = *self.stream.fill_buf().map_err(|e| bad_io(line, e))?
                        .get(0).ok_or_else(|| bad_eof(line))?;
                    self.stream.consume(1);
                    match c {
                        b'\n' => {
                            self.line += 1;
                            let line = self.line;
                            let c = self.stream.fill_buf().map_err(|e| bad_io(line, e))?.get(0);
                            match c {
                                Some(b' ') | Some(b'\t') => {
                                    self.stream.consume(1);
                                    continue;
                                }
                                _ => break b'\n',
                            }
                        }
                        c => break c,
                    }
                }
                c => break c,
            }
        })
    }

    fn read_identifier(&mut self) -> Result<Vec<u8>> {
        let mut name = Vec::new();
        Ok(loop {
            match self.peek()? {
                c @ b'-' | c @ b'A'..=b'Z' | c @ b'a'..=b'z' | c @ b'0'..=b'9' => {
                    name.push(c);
                    self.stream.consume(1);
                }
                _ => break name,
            }
        })
    }

    fn read_escaped(&mut self) -> Result<u8> {
        self.stream.consume(1);
        Ok(match self.peek()? {
            b'n' => b'\n',
            c => {
                self.stream.consume(1);
                c
            }
        })
    }

    fn read_quoted(&mut self) -> Result<Vec<u8>> {
        let mut param_value = Vec::new();
        Ok(loop {
            match self.peek()? {
                b'"' => break param_value,
                b'\\' => param_value.push(self.read_escaped()?),
                c => {
                    param_value.push(c);
                    self.stream.consume(1);
                }
            }
        })
    }

    fn read_param_value(&mut self) -> Result<String> {
        let param_value = if self.peek()? == b'"' {
            self.stream.consume(1);
            let param_value = self.read_quoted()?;
            self.stream.consume(1);
            param_value
        } else {
            let mut param_value = Vec::new();
            loop {
                match self.peek()? {
                    b',' | b';' | b':' => break,
                    b'\\' => param_value.push(self.read_escaped()?),
                    c => {
                        param_value.push(c);
                        self.stream.consume(1);
                    }
                }
            }
            param_value
        };
        let line = self.line;
        String::from_utf8(param_value).map_err(|e| bad_encoding(line, e))
    }

    fn read_params(&mut self) -> Result<Vec<Param>> {
        let line = self.line;
        let mut params = Vec::new();
        Ok('params: loop {
            let c = self.peek()?;
            self.stream.consume(1);
            match c {
                b';' => {
                    let param_name = self.read_identifier()?;
                    let mut param_values = Vec::new();
                    'param_values: loop {
                        param_values.push(self.read_param_value()?);
                        match self.peek()? {
                            b',' => continue,
                            b';' => break 'param_values,
                            b':' => break 'params params,
                            _ => unreachable!(),
                        }
                    }
                    let name = ParamName::from_bytes(&param_name).map_err(|e| bad_param(line, e))?;
                    params.push(Param { name, values: param_values });
                }
                b':' => break params,
                _ => unreachable!(),
            }
        })
    }

    fn read_value(&mut self) -> Result<String> {
        let mut value = Vec::new();
        loop {
            match self.peek()? {
                b'\n' => break,
                b'\\' => value.push(self.read_escaped()?),
                c => {
                    value.push(c);
                    self.stream.consume(1);
                }
            }
        }
        let line = self.line;
        String::from_utf8(value).map_err(|e| bad_encoding(line, e))
    }
}
