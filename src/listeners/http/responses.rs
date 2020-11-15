use crate::mpd_protocol::HandlerError;
use hyper::header::CONTENT_TYPE;
use hyper::{Body, Response, StatusCode};
use log::warn;
use serde::Serialize;

pub type GenericError = Box<dyn std::error::Error + Send + Sync>;
pub type Result = std::result::Result<Response<Body>, GenericError>;

pub fn handle_error(err: GenericError) -> Result {
    if let Some(err) = err.downcast_ref::<HandlerError>() {
        warn!["Handler error: {:?}", err];
        return internal_error();
    }
    warn!["Unknown error: {:?}", err];
    internal_error()
}

pub fn internal_error() -> Result {
    Ok(Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body("Internal error".into())
        .unwrap())
}

pub fn not_found() -> Result {
    Ok(Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body("Not Found".into())
        .unwrap())
}

pub fn ok_empty() -> Result {
    Ok(Response::builder()
        .status(StatusCode::NO_CONTENT)
        .body(Body::empty())
        .unwrap())
}

pub fn ok_json<T>(body: &T) -> Result
where
    T: ?Sized + Serialize,
{
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "application/json")
        .body(serde_json::to_vec_pretty(body)?.into())
        .unwrap())
}
