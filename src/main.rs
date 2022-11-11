use anyhow::*;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Client, Request, Response, Server};
use std::net::SocketAddr;
// Atomic reference counter
use std::sync::{Arc, RwLock};

// Guidelines for Web Content Transformation Proxies 1.0
// https://www.w3.org/TR/ct-guidelines/

fn mutate_request(req: &mut Request<Body>) -> Result<()> {
    // these header will be added again once the request get sent to the destination
    // we need to remove them to prevent duplication
    // https://www.mnot.net/blog/2011/07/11/what_proxies_must_do
    for key in &[
        "content-length",
        "transfer-encoding",
        "accept-encoding",
        "content-encoding",
    ] {
        req.headers_mut().remove(*key);
    }

    let uri = req.uri();
    let uri_string = match uri.query() {
        None => format!("https://github.com{}", uri.path()),
        Some(query) => format!("https://github.com{}?{}", uri.path(), query),
    };
    *req.uri_mut() = uri_string
        .parse()
        .context("Parsing URI in mutate_requests")?;
    // panic!("uti_string {:?}", uri_string);
    Ok(())
}

#[derive(Debug)]
struct Stats {
    proxied: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_native_roots()
        .https_or_http()
        .enable_http1()
        .build();

    let client: Client<_, hyper::Body> = Client::builder().build(https);
    let client = Arc::new(client);
    let stats: Arc<RwLock<Stats>> = Arc::new(RwLock::new(Stats { proxied: 0 }));
    // let url = "http://localhost:3000".parse().context("Parsing URL")?;

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));

    // the client cannot be guaranteed to live long enough
    let make_svc = make_service_fn(move |_| {
        let client = Arc::clone(&client);
        let stats = Arc::clone(&stats);
        async move {
            Ok::<_>(service_fn(move |mut req| {
                let client = Arc::clone(&client);
                let stats = Arc::clone(&stats);
                async move {
                    if req.uri().path() == "/status" {
                        let body: Body = format!("{:?}", stats.read().unwrap()).into();
                        Ok(Response::new(body))
                    } else {
                        println!("Proxied: {}", req.uri().path());
                        stats.write().unwrap().proxied += 1;
                        mutate_request(&mut req)?;
                        client
                            .request(req)
                            .await
                            .context("Making request to backend server")
                    }
                }
            }))
        }
    });

    Server::bind(&addr)
        .serve(make_svc)
        .await
        .context("Running server")?;
    // Ok::<(), anyhow::Error>(())

    // println!("Sleeping");
    // tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    // println!("Finished sleeping");
    // let res = client.get(url).await.context("Performing HTTP request")?;

    // println!("{:?}", res);
    // let body_bytes = hyper::body::to_bytes(res.into_body()).await.context("Getting bytes out of body")?;
    // println!("body: {:?}", body_bytes);

    // server.await??;

    Ok(())
}
