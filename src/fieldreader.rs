//! implement AsyncRead over actic_multipart::Field Stream trait

use actix_multipart::Field;
use actix_web::web::{Buf, Bytes};
use futures::{
    io::{AsyncBufRead, AsyncRead},
    stream::Stream,
    task::{Context, Poll},
};
use log::{debug, error, info};
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

        // take ownership of the chunk leaving a None in self.chunk
        let chunk = self.chunk.take();

        // we already have a chunk available
        if let Some(mut chunk) = chunk {
            // fill buf with as much chunk data or just copy the remaining chunk bytes
            let len = std::cmp::min(buf.len(), chunk.remaining());
            let slice = chunk.slice(..len);
            return match buf.write(slice.bytes()) {
                Ok(len) => {
                    debug!("wrote {} buffered bytes", len);
                    // advance the chunk by the number of written bytes
                    chunk.advance(len);
                    if chunk.has_remaining() {
                        // move back the chunk into the fieldreader
                        self.chunk = Some(chunk);
                        // immediately schedule a new poll_read as we still have some remaining data
                        cx.waker().clone().wake();
                    }
                    Poll::Ready(Ok(len))
                }
                Err(err) => {
                    info!("error {:?}", err);
                    Poll::Ready(Err(err))
                }
            };
        // no available chunk so we have to poll the field's stream first
        } else {
            return match self.as_mut().project().field.poll_next(cx) {
                // stream data available so just write as much as possible and anounce readyness
                Poll::Ready(Some(Ok(mut chunk))) => {
                    info!("received {} bytes", chunk.len());
                    match buf.write(chunk.bytes()) {
                        Ok(len) => {
                            debug!("wrote {} bytes", len);
                            // if some chunk data is remaining
                            if len < chunk.len() {
                                // advance the chunk and move it into the struct
                                chunk.advance(len);
                                self.chunk = Some(chunk);
                            }
                            Poll::Ready(Ok(len))
                        }
                        Err(err) => {
                            error!("error {:?}", err);
                            Poll::Ready(Err(err))
                        }
                    }
                }
                // normally unpack only request needed bytes by sizing buf to needed bytes,
                // so we don't fall into this case
                Poll::Ready(None) => {
                    debug!("end of stream");
                    Poll::Ready(Ok(0))
                }
                Poll::Ready(Some(Err(err))) => {
                    error!("error {:?}", err);
                    Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, "error")))
                }
                // poll_ready has already been scheduled again by field.poll_next at this point so
                // just return pending
                Poll::Pending => Poll::Pending,
            };
        }
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
                let buf = self.project().chunk.as_ref().unwrap().bytes();
                return Poll::Ready(Ok(buf));
            } else {
                return match self.as_mut().project().field.poll_next(cx) {
                    Poll::Ready(Some(Ok(chunk))) => {
                    	info!("received {} bytes", chunk.len());
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
