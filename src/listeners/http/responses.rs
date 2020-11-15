use crate::mpd_protocol::InputError;
use hyper::header::CONTENT_TYPE;
use hyper::{Body, Response, StatusCode};
use log::{debug, warn};
use serde::Serialize;

pub type GenericError = Box<dyn std::error::Error + Send + Sync>;
pub type Result = std::result::Result<Response<Body>, GenericError>;

pub fn handle_error(err: GenericError) -> Result {
    if let Some(err) = err.downcast_ref::<InputError>() {
        debug!["Input error: {:?}", err];
        return Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(format!["{:?}", err].into())
            .unwrap());
    }
    warn!["Handler error: {:?}", err];
    Ok(Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(format!["{:?}", err].into())
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
