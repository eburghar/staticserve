use	crate::config::Config;

use	actix_service::{Service, Transform};
use	actix_web::{
	dev::{ServiceRequest, ServiceResponse},
	error::ErrorUnauthorized,
	web::Data,
	Error,
};
use	futures::future::{err, ok, Either, Ready};
use	std::task::{Context, Poll};

// TokenAuth is	an empty struct	on which to	implement the Transform	trait (factory)	which is
// responsible to instanciate the Middleware service with the next service embeded
pub	struct TokenAuth;

// Middleware factory is `Transform` trait from	actix-service crate
// `S` - type of the next service
// `B` - type of response's	body
impl<S,	B> Transform<S>	for	TokenAuth
where
	S: Service<Request = ServiceRequest, Response =	ServiceResponse<B>,	Error =	Error>,
	S::Future: 'static,
	B: 'static,
{
	type Request = ServiceRequest;
	type Response =	ServiceResponse<B>;
	type Error = Error;
	type InitError = ();
	type Transform = TokenAuthMiddleware<S>;
	type Future	= Ready<Result<Self::Transform,	Self::InitError>>;

	fn new_transform(&self,	service: S)	-> Self::Future	{
		ok(TokenAuthMiddleware { service })
	}
}

// This	is the simplest	middleware possible.
pub	struct TokenAuthMiddleware<S> {
	service: S,
}

impl<S,	B> Service for TokenAuthMiddleware<S>
where
	S: Service<Request = ServiceRequest, Response =	ServiceResponse<B>,	Error =	Error>,
	S::Future: 'static,
{
	type Request = ServiceRequest;
	type Response =	ServiceResponse<B>;
	type Error = Error;
	type Future	= Either<S::Future,	Ready<Result<Self::Response, Self::Error>>>;

	// just	forward	readiness of embeded service
	fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>	{
		self.service.poll_ready(cx)
	}

	// Either forward call of embeded service or return	error if token not found of	doesn't	match
	fn call(&mut self, req:	ServiceRequest)	-> Self::Future	{
		if let Some(token) = req
			.headers()
			.get("token")
			.and_then(|token| token.to_str().ok())
		{
			if let Some(config)	= req.app_data::<Data<Config>>() {
				if token ==	config.token {
					return Either::Left(self.service.call(req));
				}
			}
		}
		Either::Right(err(ErrorUnauthorized("Not authorized")))
	}
}
