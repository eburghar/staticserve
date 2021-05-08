mod args;
mod config;
mod fieldreader;

use crate::{args::Opts, config::Config, fieldreader::FieldReader};

use actix_files::Files;
use actix_multipart::Multipart;
use actix_web::{web, middleware::Logger, post, App, Error, HttpResponse, HttpServer};
use async_compression::futures::bufread::ZstdDecoder;
use async_tar::Archive;
use futures::stream::TryStreamExt;
use log::info;
use sanitize_filename::sanitize;
use std::env;

#[post("/upload")]
async fn upload(mut payload: Multipart, config: web::Data<Config>) -> Result<HttpResponse, Error> {
	// iterate over multipart stream
	while let Ok(Some(field)) = payload.try_next().await {
		let filename = field
			.content_disposition()
			.filter(|cd| cd.get_name() == Some("file"))
			.map(|cd| cd.get_filename().and_then(|f| Some(sanitize(f))))
			.flatten();

		if let Some(filename) = filename {
			info!("untar {}", filename);
			if filename.ends_with(".tar") {
				Archive::new(FieldReader::new(field))
					.unpack(&config.upload_to)
					.await?
			} else if filename.ends_with("tar.zst") {
				Archive::new(ZstdDecoder::new(FieldReader::new(field)))
					.unpack(&config.upload_to)
					.await?
			}
		}
	}
	Ok(HttpResponse::Ok().into())
}

/// Serve static files on 0.0.0.0:8080
#[actix_web::main]
async fn serve(config: Config) -> std::io::Result<()> {
	env_logger::Builder::new()
		.parse_filters(
			&env::var(String::from("RUST_LOG"))
				.unwrap_or(String::from("staticserve=info,actix_web=info")),
		)
		.init();
	let addr_port = "0.0.0.0:8080";
	info!("listening on {}", addr_port);
	HttpServer::new(move || {
		App::new()
			.wrap(Logger::default())
			.data(config.clone())
			.service(upload)
			.configure(|cfg| {
				for (route, path) in &config.routes {
					println!("{} -> {}", route, path);
				}
			})
			.service(Files::new("/", &config.serve_from).show_files_listing())
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
