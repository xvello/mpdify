use hyper::server::Server;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response};
use std::convert::Infallible;
use std::net::SocketAddr;

pub struct HttpListener {
    address: SocketAddr,
}

impl HttpListener {
    pub fn new(address: String) -> Self {
        Self {
            address: address.parse().unwrap(),
        }
    }

    pub async fn run(&self) {
        let make_svc = make_service_fn(|_conn| {
            // This is the `Service` that will handle the connection.
            // `service_fn` is a helper to convert a function that
            // returns a Response into a `Service`.
            async { Ok::<_, Infallible>(service_fn(hello)) }
        });

        let server = Server::bind(&self.address).serve(make_svc);
        server.await.unwrap();
    }
}

async fn hello(_: Request<Body>) -> Result<Response<Body>, Infallible> {
    Ok(Response::new(Body::from("Hello World!")))
}
