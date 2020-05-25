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
impl Default for Property {
    fn default() -> Self { Property::End }
}

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
enum Bad {
    Eof,
    Io(std::io::Error),
    Encoding(std::string::FromUtf8Error),
    Property(Vec<u8>),
    Param(Vec<u8>),
}
impl Bad {
    /// Return true if it will definitely not be possible to lex any further data.
    fn is_unrecoverable(&self) -> bool {
        use Bad::*;
        match self {
            Eof | Io(_) => true,
            Encoding(_) | Property(_) | Param(_) => false,
        }
    }
    fn at(self, line: usize) -> Error {
        Error { condition: self, line }
    }
}

pub struct Error {
    condition: Bad,
    line: usize,
}
impl Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "While parsing line {}: {:?}", self.line, self.condition)
    }
}
type Maybe<T> = std::result::Result<T, Bad>;
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone)]
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

#[derive(Default, Clone)]
pub struct ContentLine {
    name: Property,
    params: Vec<Param>,
    num_params: usize,
    value: String,
    line: usize,
}
impl ContentLine {
    pub fn name(&self) -> Property {
        self.name
    }

    pub fn params(&self) -> impl Iterator<Item=&Param> {
        self.params[..self.num_params].iter()
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
    content: ContentLine,
    ident_buf: Vec<u8>,
    line: usize,
}

impl<S: BufRead> Lexer<S> {
    pub fn new(stream: S) -> Self {
        let content = ContentLine { name: Property::End, params: Vec::new(), value: String::new(), line: 0, num_params: 0 };
        let ident_buf = Vec::new();
        let line = 1;
        Self { stream, content, ident_buf, line }
    }

    pub fn lex_content_line(&mut self) -> Result<Option<&mut ContentLine>> {
        let line = self.line;
        if self.stream.fill_buf().map_err(|e| Bad::Io(e).at(line))?.is_empty() {
            return Ok(None);
        }
        // Take buffers, operate, restore buffers (even if error), return result.
        let mut content = std::mem::replace(&mut self.content, Default::default());
        let mut ident_buf = std::mem::replace(&mut self.ident_buf, Default::default());
        let mut value_buf = std::mem::replace(&mut content.value, Default::default()).into_bytes();
        let result = self.do_lex_content_line(&mut ident_buf, &mut value_buf, &mut content.params);
        let value = match String::from_utf8(value_buf) {
            Ok(k) => k,
            Err(e) => {
                let mut buf = e.into_bytes();
                buf.clear();
                String::from_utf8(buf).unwrap()
            }
        };
        content.value = value;
        self.content = content;
        self.ident_buf = ident_buf;
        match result {
            Ok(name) => {
                self.content.name = name;
                Ok(Some(&mut self.content))
            }
            Err(e) => {
                if !e.is_unrecoverable() {
                    // TODO: run out the rest of the line...
                }
                Err(e.at(line))
            }
        }
    }

    fn do_lex_content_line(&mut self, ident_buf: &mut Vec<u8>, value_buf: &mut Vec<u8>, params: &mut Vec<Param>) -> Maybe<Property> {
        self.read_identifier(ident_buf)?;
        let name = Property::from_bytes(ident_buf).map_err(|e| Bad::Property(e.to_owned()))?;
        self.read_params(params, ident_buf)?;
        self.read_value(value_buf)?;
        Ok(name)
    }

    pub fn finish(self) -> S {
        self.stream
    }
}

impl<S: BufRead> Lexer<S> {
    /// Get the next octet, handling "unfolding" and normalization of raw line breaks from CRLF to LF.
    fn peek(&mut self) -> Maybe<u8> {
        Ok(loop {
            let c = *self.stream.fill_buf().map_err(Bad::Io)?.get(0).ok_or(Bad::Eof)?;
            match c {
                b'\r' => {
                    self.stream.consume(1);
                    let c = *self.stream.fill_buf().map_err(Bad::Io)?.get(0).ok_or(Bad::Eof)?;
                    self.stream.consume(1);
                    match c {
                        b'\n' => {
                            self.line += 1;
                            let c = self.stream.fill_buf().map_err(Bad::Io)?.get(0);
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

    fn read_identifier(&mut self, ident_buf: &mut Vec<u8>) -> Maybe<()> {
        ident_buf.clear();
        Ok(loop {
            match self.peek()? {
                c @ b'-' | c @ b'A'..=b'Z' | c @ b'a'..=b'z' | c @ b'0'..=b'9' => {
                    ident_buf.push(c);
                    self.stream.consume(1);
                }
                _ => break,
            }
        })
    }

    fn read_escaped(&mut self) -> Maybe<u8> {
        self.stream.consume(1);
        Ok(match self.peek()? {
            b'n' => b'\n',
            c => {
                self.stream.consume(1);
                c
            }
        })
    }

    fn read_quoted(&mut self) -> Maybe<Vec<u8>> {
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

    fn read_param_value(&mut self) -> Maybe<String> {
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
        String::from_utf8(param_value).map_err(Bad::Encoding)
    }

    fn read_params(&mut self, params: &mut Vec<Param>, ident_buf: &mut Vec<u8>) -> Maybe<usize> {
        let mut i = 0;
        params.clear();
        'params: loop {
            let c = self.peek()?;
            self.stream.consume(1);
            match c {
                b';' => {
                    self.read_identifier(ident_buf)?;
                    let name = ParamName::from_bytes(ident_buf).map_err(|e| Bad::Param(e.to_owned()))?;
                    if let Some(param) = params.get_mut(i) {
                        param.name = name;
                        param.values.clear();
                    } else {
                        params.push(Param { name, values: Vec::new() });
                    }
                    'param_values: loop {
                        let param_value = self.read_param_value()?;
                        params[i].values.push(param_value);
                        match self.peek()? {
                            b',' => continue,
                            b';' => break 'param_values,
                            b':' => break 'params,
                            _ => unreachable!(),
                        }
                    }
                    i += 1;
                }
                b':' => break 'params,
                _ => unreachable!(),
            }
        }
        Ok(i)
    }

    fn read_value(&mut self, value_buf: &mut Vec<u8>) -> Maybe<()> {
        value_buf.clear();
        loop {
            match self.peek()? {
                b'\n' => break,
                b'\\' => {
                    let c = self.read_escaped()?;
                    value_buf.push(c);
                }
                c => {
                    value_buf.push(c);
                    self.stream.consume(1);
                }
            }
        }
        Ok(())
    }
}
