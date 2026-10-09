//! Serialization sink bounds bytes before forwarding them to the destination.
use std::io::{self, Write};

pub(crate) struct Sink<W> {
    inner: W,
    pub written: usize,
    limit: usize,
    pub exceeded: bool,
    pub failure: Option<io::Error>,
}

impl<W: Write> Sink<W> {
    pub fn new(inner: W, limit: usize) -> Self {
        Self {
            inner,
            written: 0,
            limit,
            exceeded: false,
            failure: None,
        }
    }
}
impl<W: Write> Write for Sink<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit - self.written {
            self.exceeded = true;
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "serialization limit exceeded",
            ));
        }
        let n = self.inner.write(bytes).map_err(|error| {
            let kind = error.kind();
            self.failure = Some(error);
            io::Error::new(kind, "serialization destination failed")
        })?;
        self.written += n;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn short_writes_count_exactly_and_limits_stop_before_forwarding() {
        struct Short(Vec<u8>);
        impl Write for Short {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                let len = bytes.len().min(2);
                self.0.extend_from_slice(&bytes[..len]);
                Ok(len)
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut sink = Sink::new(Short(Vec::new()), 5);
        sink.write_all(b"hello").unwrap();
        assert_eq!(sink.written, 5);
        assert!(sink.write_all(b"!").is_err());
        assert_eq!(sink.inner.0, b"hello");
        assert!(sink.exceeded);
    }
    #[test]
    fn serialization_preserves_original_destination_error() {
        struct Failed;
        impl Write for Failed {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "original destination error",
                ))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut sink = Sink::new(Failed, 100);
        assert!(serde_json::to_writer(&mut sink, &"test").is_err());
        let error = sink.failure.unwrap();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(error.to_string(), "original destination error");
        assert!(!sink.exceeded);
    }
}
