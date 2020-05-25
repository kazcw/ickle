use std::io::BufRead;

pub trait Handler {
    fn handle(&mut self, name: Vec<u8>, params: Vec<(Vec<u8>, Vec<String>)>, value: String);
}

#[derive(Debug)]
enum Condition {
    UnexpectedEof,
    Io(std::io::Error),
    Encoding(std::string::FromUtf8Error),
}
pub struct Error {
    condition: Condition,
    line: usize,
}
pub type Result<T> = std::result::Result<T, Error>;

pub struct Parser<'s, S> {
    stream: &'s mut S,
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

impl<'s, S: BufRead> Parser<'s, S> {
    pub fn new(stream: &'s mut S) -> Self {
        let line = 1;
        Self { stream, line }
    }

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

    fn read_name(&mut self) -> Result<Vec<u8>> {
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

    fn read_params(&mut self) -> Result<Vec<(Vec<u8>, Vec<String>)>> {
        let mut params = Vec::new();
        Ok('params: loop {
            let c = self.peek()?;
            self.stream.consume(1);
            match c {
                b';' => {
                    let param_name = self.read_name()?;
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
                    params.push((param_name, param_values));
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

    fn parse(mut self, handler: &mut impl Handler) -> Result<()> {
        let mut line = self.line;
        while !self.stream.fill_buf().map_err(|e| bad_io(line, e))?.is_empty() {
            let name = self.read_name()?;
            let params = self.read_params()?;
            let value = self.read_value()?;
            handler.handle(name, params, value);
            line = self.line;
        }
        Ok(())
    }
}

pub fn parse(stream: &mut impl BufRead, handler: &mut impl Handler) -> Result<()> {
    let parser = Parser::new(stream);
    parser.parse(handler)
}