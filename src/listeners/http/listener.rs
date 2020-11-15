use crate::listeners::http::responses::*;
use crate::mpd_protocol::{Command, HandlerInput, HandlerOutput, HandlerResult};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Server};
use log::debug;
use std::net::SocketAddr;
use std::str::Split;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;

type Handler = Arc<Sender<HandlerInput>>;

pub struct HttpListener {
    address: SocketAddr,
    handlers: Handler,
}

impl HttpListener {
    pub fn new(address: String, handler: Sender<HandlerInput>) -> Self {
        Self {
            address: address.parse().unwrap(),
            handlers: Handler::new(handler),
        }
    }

    pub async fn run(&self) {
        let new_service = make_service_fn(move |_| {
            let h = self.handlers.clone();
            async { Ok::<_, GenericError>(service_fn(move |req| handle_request(req, h.clone()))) }
        });

        let server = Server::bind(&self.address).serve(new_service);
        debug!("Listening on http://{}", &self.address);
        server.await.unwrap();
    }
}

async fn handle_request(req: Request<Body>, handler: Handler) -> Result {
    if !req.uri().path().starts_with('/') {
        return not_found();
    }
    let mut path_parts = req.uri().path()[1..].split('/');

    match match path_parts.next() {
        Some("command") => handle_command(handler, path_parts).await,
        _ => not_found(),
    } {
        Ok(result) => Ok(result),
        Err(err) => handle_error(err),
    }
}

async fn handle_command(handler: Handler, input: Split<'_, char>) -> Result {
    let command = Command::from_tokens(input)?;
    match execute_command(handler, command).await? {
        HandlerOutput::Data(data) => ok_json(&data),
        HandlerOutput::Ok => ok_empty(),
        HandlerOutput::Close => ok_empty(),
    }
}

async fn execute_command(handler: Handler, command: Command) -> HandlerResult {
    let (tx, rx) = oneshot::channel();
    handler.send(HandlerInput { command, resp: tx }).await?;
    rx.await.unwrap()
}
