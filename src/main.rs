mod args;
mod config;

use crate::{args::Opts, config::Config};

use actix_files::Files;
use actix_multipart::{Field, Multipart};
use actix_web::{
    middleware::Logger,
    post,
    web::{Buf, Bytes},
    App, Error, HttpResponse, HttpServer,
};
use async_tar::Archive;
use futures::{
    io::AsyncRead,
    stream::{Stream, TryStreamExt},
    task::{Context, Poll},
};
use log::{debug, error, info};
use pin_project_lite::pin_project;
use sanitize_filename::sanitize;
use std::{io::Write, pin::Pin};

pin_project! {
    pub struct FieldReader {
        #[pin]
        field: Field,
        chunk: Option<Bytes>,
        // start position in the chunk TODO: use a Cursor
        pos: usize
    }
}

impl FieldReader {
    fn new(field: Field) -> Self {
        FieldReader {
            field,
            chunk: None,
            pos: 0,
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

        let chunk = self.chunk.take();

        // we already have an available chunk
        if let Some(chunk) = chunk {
            // len is either the size of the poll buffer or the remaining bytes in the chunk
            let len = std::cmp::min(buf.len(), chunk.len() - self.pos);
            let slice = chunk.slice(self.pos..self.pos + len);
            return match buf.write(slice.bytes()) {
                Ok(len) => {
                    debug!("wrote {} buffered bytes from {}", len, self.pos);
                    self.pos += len;
                    if self.pos == chunk.len() {
                        debug!("Chunk has been consumed");
                        self.pos = 0;
                    } else {
                        // move back the chunk into the fieldreader
                        self.chunk = Some(chunk);
                        // immediately schedule a new poll_read as we still have data
                        cx.waker().clone().wake();
                    }
                    Poll::Ready(Ok(len))
                }
                Err(err) => {
                    info!("error {:?}", err);
                    Poll::Ready(Err(err))
                }
            };
        // no available chunk so we have to poll the field's stream
        } else {
            return match self.as_mut().project().field.poll_next(cx) {
                Poll::Ready(Some(Ok(chunk))) => {
                    let len = chunk.len();
                    info!("received {} bytes", len);
                    match buf.write(chunk.bytes()) {
                        Ok(len) => {
                            debug!("wrote {} bytes", len);
                            self.chunk = Some(chunk);
                            self.pos = len;
                            Poll::Ready(Ok(len))
                        }
                        Err(err) => {
                            error!("error {:?}", err);
                            Poll::Ready(Err(err))
                        }
                    }
                }
                // normally unpack only request needed bytes, no we don't fall into this case
                Poll::Ready(None) => {
                    debug!("end of stream");
                    Poll::Ready(Ok(0))
                }
                Poll::Ready(Some(Err(err))) => {
                    error!("error {:?}", err);
                    Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, "error")))
                }
                Poll::Pending => Poll::Pending,
            };
        }
    }
}

#[post("/upload")]
async fn upload(mut payload: Multipart) -> Result<HttpResponse, Error> {
    // iterate over multipart stream
    while let Ok(Some(field)) = payload.try_next().await {
        if let Some(content_type) = field.content_disposition() {
            match content_type.get_name() {
                Some("file") => {
                    if let Some(filename) = content_type.get_filename() {
                        let sane_file = sanitize(&filename);
                        info!("untar {}", sane_file);
                        let reader = FieldReader::new(field);
                        let archive = Archive::new(reader);
                        archive.unpack("archive").await?;
                    }
                }
                _ => (),
            }
        }
    }
    Ok(HttpResponse::Ok().into())
}

/// Serve static files on 0.0.0.0:8080
#[actix_web::main]
async fn serve(config: Config) -> std::io::Result<()> {
    let addr_port = "0.0.0.0:8080";
    std::env::set_var("RUST_LOG", "staticserve=debug,actix_web=info");
    env_logger::init();
    info!("listening on {}", addr_port);
    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .data(config.clone())
            .service(upload)
            .service(Files::new("/", ".").show_files_listing())
    })
    .bind(addr_port)?
    .run()
    .await
}

fn main() -> anyhow::Result<()> {
    let opts: Opts = argh::from_env();
    // read yaml config
    let config = Config::read(&opts.config)?;
    serve(config).unwrap();
    Ok(())
}
