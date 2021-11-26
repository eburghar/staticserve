use actix_service::{Service, Transform};
use actix_web::{
	dev::{ServiceRequest, ServiceResponse},
	http::header::{HeaderValue, CACHE_CONTROL},
	Error,
};
use futures::future::{ok, FutureExt, LocalBoxFuture, Ready};
use serde::Deserialize;
use std::{
	collections::BTreeMap,
	rc::Rc,
	task::{Context, Poll},
};

#[derive(Deserialize, Clone)]
/// Control Cache behavior
pub struct CacheControl {
	// cache control instructions for paths matching a list prefix
	pub prefixes: Option<BTreeMap<String, String>>,
	// cache control instructions for paths matching a list suffix
	pub suffixes: Option<BTreeMap<String, String>>,
}

impl CacheControl {
	/// return the first cache-control value that match path as a prefix or as a suffix
	fn get_value(&self, path: &str) -> Option<&str> {
		if let Some(ref prefixes) = self.prefixes {
			for (prefix, value) in prefixes.iter() { if path.starts_with(prefix) {
					return Some(value);
				}
			}
		}
		if let Some(ref suffixes) = self.suffixes {
			for (suffix, value) in suffixes.iter() {
				if path.ends_with(suffix) {
					return Some(value);
				}
			}
		}
		None
	}
}

impl Default for CacheControl {
	fn default() -> Self {
		Self {
			prefixes: None,
			suffixes: None,
		}
	}
}

#[derive(Clone)]
pub struct CacheHeaders(Rc<CacheControl>);

impl Default for CacheHeaders {
	fn default() -> Self {
		Self(Rc::new(CacheControl::default()))
	}
}

impl CacheHeaders {
	/// Construct `CacheHeaders` middleware.
	pub fn new(cache: CacheControl) -> Self {
		Self(Rc::new(cache))
	}
}

// Middleware factory is `Transform` trait from actix-service crate
// `S` - type of the next service
// `B` - type of response's body
impl<S, B> Transform<S> for CacheHeaders
where
	S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
	S::Future: 'static,
	B: 'static,
{
	type Request = ServiceRequest;
	type Response = ServiceResponse<B>;
	type Error = Error;
	type InitError = ();
	type Transform = CacheHeadersMiddleware<S>;
	type Future = Ready<Result<Self::Transform, Self::InitError>>;

	fn new_transform(&self, service: S) -> Self::Future {
		ok(CacheHeadersMiddleware {
			service,
			inner: self.0.clone(),
		})
	}
}

pub struct CacheHeadersMiddleware<S> {
	service: S,
	inner: Rc<CacheControl>,
}

impl<S, B> Service for CacheHeadersMiddleware<S>
where
	S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
	S::Future: 'static,
{
	type Request = ServiceRequest;
	type Response = ServiceResponse<B>;
	type Error = Error;
	type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

	fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		self.service.poll_ready(cx)
	}

	fn call(&mut self, req: ServiceRequest) -> Self::Future {
		let inner = self.inner.clone();
		let path = req.path().to_owned();
		let fut = self.service.call(req);

		async move {
			let mut res = fut.await?;
			if let Some(cache_control) = inner.get_value(&path) {
				res.headers_mut()
					.insert(CACHE_CONTROL, HeaderValue::from_str(cache_control)?);
			} else {
				res.headers_mut()
					.insert(CACHE_CONTROL, HeaderValue::from_static("private,max-age=0"));
			}
			Ok(res)
		}
		.boxed_local()
	}
}
