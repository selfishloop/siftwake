use crate::{Error, Result};
use std::io::BufRead;
fn invalid(message: &str) -> Error {
    Error::Corrupt(format!("streaming JSON: {message}"))
}
pub(crate) struct Input<R> {
    pub reader: R,
    pub scratch: Vec<u8>,
    pub limit: usize,
}
impl<R: BufRead> Input<R> {
    pub fn peek(&mut self) -> Result<Option<u8>> {
        Ok(self.reader.fill_buf()?.first().copied())
    }
    pub fn byte(&mut self) -> Result<u8> {
        let byte = self.peek()?.ok_or_else(|| invalid("unexpected EOF"))?;
        self.reader.consume(1);
        Ok(byte)
    }
    pub fn whitespace(&mut self) -> Result<()> {
        while self
            .peek()?
            .is_some_and(|b| matches!(b, b' ' | b'\r' | b'\n' | b'\t'))
        {
            self.byte()?;
        }
        Ok(())
    }
    pub fn expect(&mut self, expected: u8) -> Result<()> {
        self.whitespace()?;
        if self.byte()? != expected {
            return Err(invalid("invalid delimiter"));
        }
        Ok(())
    }
    pub fn push(&mut self, byte: u8) -> Result<()> {
        if self.scratch.len() == self.limit {
            return Err(invalid("entry exceeds configured byte limit"));
        }
        if self.scratch.len() == self.scratch.capacity() {
            let target = self
                .scratch
                .capacity()
                .max(256)
                .saturating_mul(2)
                .min(self.limit);
            self.scratch.reserve_exact(target - self.scratch.len());
        }
        self.scratch.push(byte);
        Ok(())
    }
    pub fn value<T: serde::de::DeserializeOwned>(&mut self) -> Result<T> {
        self.whitespace()?;
        self.scratch.clear();
        let first = self.byte()?;
        self.push(first)?;
        let composite = matches!(first, b'{' | b'[');
        let mut depth = usize::from(composite);
        let mut quoted = first == b'"';
        let mut escape = false;
        if composite || quoted {
            loop {
                let bytes = self.reader.fill_buf()?;
                if bytes.is_empty() {
                    return Err(invalid("unexpected EOF"));
                }
                let mut consumed = 0;
                for &byte in bytes {
                    consumed += 1;
                    if quoted {
                        if escape {
                            escape = false;
                        } else if byte == b'\\' {
                            escape = true;
                        } else if byte == b'"' {
                            quoted = false;
                        }
                    } else {
                        match byte {
                            b'"' => quoted = true,
                            b'{' | b'[' => {
                                depth += 1;
                                if depth > 128 {
                                    return Err(invalid("JSON nesting limit"));
                                }
                            }
                            b'}' | b']' => {
                                depth = depth
                                    .checked_sub(1)
                                    .ok_or_else(|| invalid("unbalanced JSON"))?
                            }
                            _ => {}
                        }
                    }
                    if !quoted && depth == 0 {
                        break;
                    }
                }
                let required = self
                    .scratch
                    .len()
                    .checked_add(consumed)
                    .ok_or_else(|| invalid("entry length overflow"))?;
                if required > self.limit {
                    return Err(invalid("entry exceeds configured byte limit"));
                }
                if required > self.scratch.capacity() {
                    let capacity = required
                        .checked_next_power_of_two()
                        .unwrap_or(self.limit)
                        .min(self.limit);
                    self.scratch.reserve_exact(capacity - self.scratch.len());
                }
                self.scratch.extend_from_slice(&bytes[..consumed]);
                self.reader.consume(consumed);
                if !quoted && depth == 0 {
                    break;
                }
            }
        } else {
            while self
                .peek()?
                .is_some_and(|b| !matches!(b, b',' | b'}' | b']' | b' ' | b'\r' | b'\n' | b'\t'))
            {
                let byte = self.byte()?;
                self.push(byte)?;
            }
        }
        serde_json::from_slice(&self.scratch).map_err(|e| invalid(&e.to_string()))
    }
    pub fn array<T: serde::de::DeserializeOwned>(
        &mut self,
        mut item: impl FnMut(T) -> Result<()>,
    ) -> Result<()> {
        self.expect(b'[')?;
        self.whitespace()?;
        if self.peek()? == Some(b']') {
            self.byte()?;
            return Ok(());
        }
        loop {
            item(self.value()?)?;
            self.whitespace()?;
            match self.byte()? {
                b']' => return Ok(()),
                b',' => {}
                _ => return Err(invalid("invalid array separator")),
            }
        }
    }
}
