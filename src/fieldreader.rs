//! implement AsyncRead over actic_multipart::Field Stream trait

use actix_multipart::Field;
use actix_web::web::{Buf, Bytes};
use futures::{
    io::AsyncRead,
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
        FieldReader {
            field,
            chunk: None,
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
                    if chunk.remaining() != 0 {
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
