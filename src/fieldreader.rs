//! implement AsyncRead over actix_multipart::Field (Stream).
//! Code borrowed  from tokio_util::io::StreamReader

use actix_multipart::Field;
use actix_web::web::{Buf, Bytes};
use futures::{
    io::{AsyncBufRead, AsyncRead},
    stream::Stream,
    task::{Context, Poll},
};
use log::{debug, info};
use pin_project_lite::pin_project;
use std::{io::Write, pin::Pin};

pin_project! {
    pub struct FieldReader {
        #[pin]
        field: Field,
        chunk: Option<Bytes>,
    }
}

impl FieldReader {
    pub fn new(field: Field) -> Self {
        FieldReader { field, chunk: None }
    }

    /// Do we have a chunk and is it non-empty?
    fn has_chunk(self: Pin<&mut Self>) -> bool {
        if let Some(chunk) = self.project().chunk {
            chunk.has_remaining()
        } else {
            false
        }
    }
}

impl AsyncRead for FieldReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        mut buf: &mut [u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        debug!("poll_read into {} bytes", buf.len());
        // get a new Pin<&mut FieldReader> otherwise self would be consumed by calling poll_fill_buf
        let inner_buf = match self.as_mut().poll_fill_buf(cx) {
            Poll::Ready(Ok(buf)) => buf,
            Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
            Poll::Pending => return Poll::Pending,
        };
        // fill buf entirely with field's chunk or fill partially with field's chunk remaining data
        let len = std::cmp::min(inner_buf.len(), inner_buf.remaining());
        return match buf.write(&inner_buf[..len]) {
            Ok(len) => {
                debug!("consumed {} buffered bytes", len);
                // advance cursor of internal bytes
                self.consume(len);
                Poll::Ready(Ok(len))
            }
            Err(err) => Poll::Ready(Err(err)),
        };
    }
}

impl AsyncBufRead for FieldReader {
    fn poll_fill_buf(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<&[u8], std::io::Error>> {
        loop {
            if self.as_mut().has_chunk() {
                // This unwrap is very sad, but it can't be avoided.
                let buf = self.project().chunk.as_ref().unwrap();
                return Poll::Ready(Ok(buf.bytes()));
            } else {
                return match self.as_mut().project().field.poll_next(cx) {
                    Poll::Ready(Some(Ok(chunk))) => {
                        info!("received {} bytes from stream", &chunk.remaining());
                        // Go around the loop in case the chunk is empty.
                        *self.as_mut().project().chunk = Some(chunk);
                        continue;
                    }
                    Poll::Ready(Some(Err(err))) => Poll::Ready(Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        err.to_string(),
                    ))),
                    Poll::Ready(None) => Poll::Ready(Ok(&[])),
                    Poll::Pending => Poll::Pending,
                };
            }
        }
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        if amt > 0 {
            self.project()
                .chunk
                .as_mut()
                .expect("No chunk present")
                .advance(amt);
        }
    }
}
