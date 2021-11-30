mod args;
mod config;
mod fieldreader;

use crate::{args::Opts, config::Config, fieldreader::FieldReader};

use actix_cachecontrol_middleware::middleware::CacheHeaders;
use actix_files::{Files, NamedFile};
use actix_multipart::Multipart;
use actix_token_middleware::middleware::jwtauth::JwtAuth;
use actix_web::{middleware, web, App, Error, HttpRequest, HttpResponse, HttpServer};
use anyhow::{anyhow, Context};
use async_compression::futures::bufread::ZstdDecoder;
use async_tar::Archive;
use futures::{executor, stream::TryStreamExt};
use rustls::{
	internal::pemfile::{certs, pkcs8_private_keys, rsa_private_keys},
	NoClientAuth, ServerConfig,
};
use sanitize_filename::sanitize;
use std::{
	env,
	fs::{create_dir_all, File},
	io::{BufReader, Write},
	path::Path,
	sync::{mpsc, Arc},
	thread,
};

type Sender = mpsc::Sender<()>;

#[derive(Clone)]
/// Structure to pass state through routes and resources
struct AppState {
	config: Config,
	tx: Sender,
}

const INDEX: &str = r#"<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <title>It works !</title>
  </head>
  <body>
    <p>You can now upload new content.</p>
  </body>
</html>"#;

/// upload an unpack a new archive in destination directory. the url is protected by a token
async fn upload(mut payload: Multipart, state: web::Data<AppState>) -> Result<HttpResponse, Error> {
	// iterate over multipart stream
	while let Some(field) = payload.try_next().await? {
		let filename = field
			.content_disposition()
			.filter(|cd| cd.get_name() == Some("file"))
			.map(|cd| cd.get_filename().map(|f| sanitize(f)))
			.flatten();

		if let Some(filename) = filename {
			log::info!("untar {}", filename);
			if filename.ends_with(".tar") {
				Archive::new(FieldReader::new(field))
					.unpack(&state.config.dir)
					.await?;
				let _ = state.tx.send(());
			} else if filename.ends_with("tar.zst") {
				Archive::new(ZstdDecoder::new(FieldReader::new(field)))
					.unpack(&state.config.dir)
					.await?;
				let _ = state.tx.send(());
			}
		}
	}
	Ok(HttpResponse::Ok().into())
}

/// Serve associated files for dynamic routes
async fn route_path(req: HttpRequest, state: web::Data<AppState>) -> actix_web::Result<NamedFile> {
	// we are sure that there is a match_pattern and a corresponding value in config.routes hashmap
	// because this handler has been configured from it: unwrap should never panic
	if let Some(ref routes) = state.config.routes {
		let path = req.match_pattern().unwrap();
		let file = Path::new(&state.config.root).join(routes.get(&path).unwrap());
		Ok(NamedFile::open(file)?)
	} else {
		// shouldn't panic as the service is only activated if state.config.routes != None
		panic!("routes is not configured");
	}
}

/// Serve static files on addr
async fn serve(mut config: Config, addr: String) -> anyhow::Result<bool> {
	// set keys from jwks endpoint
	if let Some(ref mut jwt) = config.jwt {
		let _ = jwt
			.set_keys()
			.await
			.map_err(|e| anyhow!("failed to get jkws keys {}", e))?;
	}

	// create directory with index.html
	if !config.root.exists() {
		create_dir_all(&config.root)
			.with_context(|| format!("unable to create directory {:?}", &config.root))?;
		let index = config.root.join("index.html");
		let mut f =
			File::create(&index).with_context(|| format!("failed to create {:?}", &index))?;
		f.write_all(INDEX.as_bytes())
			.with_context(|| format!("failed to write to {:?}", &index))?;
	}

	// channel allowing upload task to ask for a reload of the server
	let (tx, rx) = mpsc::channel::<()>();

	// copy some values before config is moved
	let tls = config.tls.clone();

	// initialize the state shared among routes and services
	let state = AppState { config: config, tx };

	// build the server
	let server = HttpServer::new(move || {
		let mut app = App::new()
			.wrap(middleware::Logger::default())
			.wrap(middleware::Compress::default())
			.wrap(middleware::Condition::new(
				state.config.cache.is_some(),
				CacheHeaders::new(state.config.cache.clone()),
			))
			.data(state.clone());
		if let Some(ref jwt) = state.config.jwt {
			app = app.service(
				web::resource("/upload")
					.wrap(JwtAuth::new(jwt.clone()))
					.route(web::post().to(upload)),
			);
		}
		if let Some(ref routes) = state.config.routes {
			app = app.configure(|cfg| {
				routes.iter().fold(cfg, |cfg, (path, _)| {
					cfg.route(&path, web::get().to(route_path))
				});
			})
		}
		app.service(Files::new("/", &state.config.root).index_file("index.html"))
	});

	// bind to http or https
	let server = if let Some(ref tls) = tls {
		// Create tls config
		let mut tls_config = ServerConfig::new(NoClientAuth::new());

		// Parse the certificate and set it in the configuration
		let crt_chain = certs(&mut BufReader::new(
			File::open(&tls.crt).with_context(|| format!("unable to read {:?}", &tls.crt))?,
		))
		.map_err(|_| anyhow!("error reading certificate"))?;

		// Parse the key in RSA or PKCS8 format
		let invalid_key = |_| anyhow!("invalid key in {:?}", &tls.key);
		let no_key = || anyhow!("no key found in {:?}", &tls.key);
		let mut keys = rsa_private_keys(&mut BufReader::new(File::open(&tls.key)?))
			.map_err(invalid_key)
			.and_then(|x| (!x.is_empty()).then(|| x).ok_or(no_key()))
			.or_else(|_| {
				pkcs8_private_keys(&mut BufReader::new(File::open(&tls.key)?))
					.map_err(invalid_key)
					.and_then(|x| (!x.is_empty()).then(|| x).ok_or(no_key()))
			})?;
		tls_config
			.set_single_cert(crt_chain, keys.remove(0))
			.with_context(|| "error setting crt/key pair")?;
		server
			.bind_rustls(&addr, tls_config)
			.with_context(|| format!("unable to bind to https://{}", &addr))?
			.run()
	} else {
		log::warn!("TLS is not activated. Use only for development");
		server
			.bind(&addr)
			.with_context(|| format!("unable to bind to http://{}", &addr))?
			.run()
	};

	// wait for reload message in a separate thread as recv call is blocking
	let srv = server.clone();
	// use an arc to know if the thread went up to completion (ie reload was triggered)
	let reloaded = Arc::new(());
	let reloaded_wk = Arc::downgrade(&reloaded);
	thread::spawn(move || {
		// void statement to move 'reloaded' to the closure
		let _ = reloaded;
		rx.recv().unwrap_or_else(|_| {});
		executor::block_on(srv.stop(true))
	});

	log::info!(
		"listening on http{}://{}",
		if tls.is_some() { "s" } else { "" },
		&addr
	);
	server.await?;

	// if the weak pointer can't upgrade then the reload recv thread is gone
	Ok(reloaded_wk.upgrade().is_none())
}

fn main() -> anyhow::Result<()> {
	// setup logging
	env_logger::Builder::new()
		.parse_filters(
			&env::var("RUST_LOG".to_owned())
				.unwrap_or("staticserve=info,actix_web=info".to_owned()),
		)
		.init();

	// read command line options
	let args: Opts = args::from_env();
	// read yaml config
	let mut config = Config::read(&args.config)?;
	// join dir and root
	config.root = config.dir.join(config.root);

	// start actix main loop
	let mut system = actix_web::rt::System::new("main");
	loop {
		let reloaded =
			system.block_on::<_, anyhow::Result<bool>>(serve(config.clone(), args.addr.clone()))?;
		if reloaded {
			log::info!("restart server");
		} else {
			log::info!("stop server");
			break;
		}
	}

	Ok(())
}
