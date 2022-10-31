mod args;
mod config;
mod fieldreader;

use crate::{
	args::Opts,
	config::{Config, HookType},
	fieldreader::FieldReader,
};

use actix_cachecontrol_middleware::middleware::CacheHeaders;
use actix_files::{Files, NamedFile};
use actix_multipart::Multipart;
use actix_token_middleware::middleware::jwtauth::JwtAuth;
use actix_web::{
	dev::{ServiceRequest, ServiceResponse},
	http::StatusCode,
	middleware,
	web::{self, Data},
	App, Error, HttpRequest, HttpResponse, HttpServer,
};
use anyhow::{anyhow, Context};
use async_compression::futures::bufread::ZstdDecoder;
use async_tar::Archive;
use futures::{executor, stream::TryStreamExt};
use rustls::{Certificate, PrivateKey, ServerConfig};
use rustls_pemfile::{certs, pkcs8_private_keys, rsa_private_keys};
use sanitize_filename::sanitize;
use std::{
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
		let filename = field.content_disposition().get_filename().map(sanitize);

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
		// trigger updated hooks
		if let Some(ref hooks) = state.config.hooks {
			hooks.trigger(HookType::Updated);
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
		jwt.set_keys()
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
	let state = AppState { config, tx };

	// build the server
	let server = HttpServer::new(move || {
		let mut app = App::new()
			.wrap(middleware::Logger::default())
			.wrap(middleware::Compress::default())
			.wrap(middleware::Condition::new(
				state.config.cache.is_some(),
				CacheHeaders::new(state.config.cache.clone()),
			))
			.app_data(Data::new(state.clone()));
		// add upload service
		if let Some(ref jwt) = state.config.jwt {
			app = app.service(
				web::resource("/upload")
					.wrap(JwtAuth::new(jwt.clone()))
					.route(web::post().to(upload)),
			);
		} else {
			log::warn!("upload is not protect by an authorization token. Use only for development");
			app = app.service(web::resource("upload").route(web::post().to(upload)));
		}
		// add configured routes
		if let Some(ref routes) = state.config.routes {
			for (path, dest) in routes.iter() {
				log::debug!("add route {} -> {}", path, dest);
				app = app.route(path, web::get().to(route_path));
			}
		}
		// add static file service
		app.service(
			Files::new("/", &state.config.root)
				// TODO: add an option in the config file
				// .use_hidden_files()
				.prefer_utf8(true)
				.default_handler(|req: ServiceRequest| async {
					let default = req
						.app_data::<web::Data<AppState>>()
						.and_then(|state| state.config.default.clone());
					let (http_req, _) = req.into_parts();
					if let Some(default) = default {
						let mut response =
							actix_files::NamedFile::open(default.file)?.into_response(&http_req);
						*response.status_mut() =
							StatusCode::from_u16(default.status).unwrap_or(StatusCode::NOT_FOUND);
						Ok(ServiceResponse::new(http_req, response))
					} else {
						Ok(ServiceResponse::new(
							http_req,
							HttpResponse::new(StatusCode::NOT_FOUND),
						))
					}
				})
				.index_file("index.html"),
		)
	});

	// bind to http or https
	let server = if let Some(ref tls) = tls {
		// Create tls config
		let config = ServerConfig::builder()
			.with_safe_defaults()
			.with_no_client_auth();

		let crt_chain = certs(&mut BufReader::new(
			File::open(&tls.crt).with_context(|| format!("unable to read {:?}", &tls.crt))?,
		))
		.map_err(|_| anyhow!("error reading certificate"))?
		.into_iter()
		.map(Certificate)
		.collect();

		// Parse the key in RSA or PKCS8 format
		let invalid_key = |_| anyhow!("invalid key in {:?}", &tls.key);
		let no_key = || anyhow!("no key found in {:?}", &tls.key);
		let mut keys: Vec<PrivateKey> =
			rsa_private_keys(&mut BufReader::new(File::open(&tls.key)?))
				.map_err(invalid_key)
				// return an error if there is no key
				.and_then(|x| (!x.is_empty()).then(|| x).ok_or_else(no_key))
				.or_else(|_| {
					pkcs8_private_keys(&mut BufReader::new(File::open(&tls.key)?))
						.map_err(invalid_key)
						// return an error if there is no key
						.and_then(|x| (!x.is_empty()).then(|| x).ok_or_else(no_key))
				})?
				.into_iter()
				.map(PrivateKey)
				.collect();
		let tls_config = config
			.with_single_cert(crt_chain, keys.swap_remove(0))
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
	let srv = server.handle();
	// use an arc to know if the thread went up to completion (ie reload was triggered)
	let reloaded = Arc::new(());
	let reloaded_wk = Arc::downgrade(&reloaded);
	thread::spawn(move || {
		// void statement to move 'reloaded' to the closure
		let _ = reloaded;
		rx.recv().unwrap_or(());
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
	// read command line options
	let args: Opts = args::from_env();

	// setup logging
	env_logger::init_from_env(
		env_logger::Env::new()
			.default_filter_or("staticserve=info,actix_web=info")
			.default_write_style_or("auto"),
	);
	log::info!("{} v{}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));

	// read yaml config
	let mut config = Config::read(&args.config)?;
	// join dir and root
	config.root = config.dir.join(config.root);
	// join root and default file
	if let Some(ref mut default) = config.default {
		default.file = config.root.clone().join(&default.file);
	}

	// start actix main loop
	let system = actix_web::rt::System::new();
	loop {
		let reloaded = system.block_on(serve(config.clone(), args.addr.clone()))?;
		if reloaded {
			log::info!("restart server");
		} else {
			log::info!("stop server");
			break;
		}
	}

	Ok(())
}
