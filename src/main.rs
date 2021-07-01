mod args;
mod config;
mod fieldreader;
mod auth;

use crate::{args::Opts, config::Config, fieldreader::FieldReader, auth::TokenAuth};

use actix_files::{Files, NamedFile};
use actix_multipart::Multipart;
use actix_web::{middleware, post, web, App, Error, HttpRequest, HttpResponse, HttpServer};
use async_compression::futures::bufread::ZstdDecoder;
use async_tar::Archive;
use futures::stream::TryStreamExt;
use log::info;
use sanitize_filename::sanitize;
use std::{env, path::Path};

/// upload an unpack a new archive in destination directory. the url is protected by a token
#[post("/upload", wrap = "TokenAuth")]
async fn upload(mut payload: Multipart, config: web::Data<Config>) -> Result<HttpResponse, Error> {
	// iterate over multipart stream
	while let Some(field) = payload.try_next().await? {
		let filename = field
			.content_disposition()
			.filter(|cd| cd.get_name() == Some("file"))
			.map(|cd| cd.get_filename().map(|f| sanitize(f)))
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

/// Serve associated files for dynamic routes
async fn route_path(req: HttpRequest, config: web::Data<Config>) -> actix_web::Result<NamedFile> {
	// we are sure than there is a match_pattern and a corresponding value in config.routes hashmap
	// because this handler has been configured from it: unwrap should never panic
	let path = req.match_pattern().unwrap();
	let file = Path::new(&config.serve_from).join(config.routes.get(&path).unwrap());
	Ok(NamedFile::open(file)?)
}

/// Serve static files on 0.0.0.0:8080
#[actix_web::main]
async fn serve(config: Config) -> std::io::Result<()> {
	env_logger::Builder::new()
		.parse_filters(
			&env::var("RUST_LOG".to_owned())
				.unwrap_or("staticserve=info,actix_web=info".to_owned()),
		)
		.init();
	let addr_port = "0.0.0.0:8080";
	info!("listening on {}", addr_port);
	HttpServer::new(move || {
		App::new()
			.wrap(middleware::Logger::default())
			.wrap(middleware::Compress::default())
			.data(config.clone())
			.service(upload)
			.configure(|cfg| {
				config.routes.iter().fold(cfg, |cfg, (path, file)| {
					println!("{} -> {}", path, file);
					cfg.route(&path, web::get().to(route_path))
				});
			})
			.service(Files::new("/", &config.serve_from).index_file("index.html"))
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
