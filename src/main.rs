mod args;
mod config;
mod fieldreader;

use crate::{args::Opts, config::Config, fieldreader::FieldReader};

use actix_files::Files;
use actix_multipart::Multipart;
use actix_web::{middleware::Logger, post, App, Error, HttpResponse, HttpServer};
use async_tar::Archive;
use futures::stream::TryStreamExt;
use log::info;
use sanitize_filename::sanitize;
use async_compression::futures::bufread::ZstdDecoder;

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
                        if sane_file.ends_with(".tar") {
                        	Archive::new(FieldReader::new(field)).unpack("archive").await?
                        }
                        else if sane_file.ends_with("tar.zst") {
	                        Archive::new(ZstdDecoder::new(FieldReader::new(field))).unpack("archive").await?
                        }
                        //let archive = Archive::new(reader);
                        //archive.unpack("archive").await?;
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
