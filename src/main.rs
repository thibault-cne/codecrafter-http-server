use std::io::{Read, Write};
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use http::{HttpCode, HttpVersion, Method};
use request::{Request, RequestBuffer};
use response::Response;
use router::{ComparePath, Route, Router};

mod http;
mod request;
mod response;
mod router;

const MAX_BUFFER_SIZE: usize = 2048;

#[tokio::main]
async fn main() {
    // You can use print statements as follows for debugging, they'll be visible when running tests.
    println!("Logs from your program will appear here!");

    let listener = TcpListener::bind("127.0.0.1:4221").await.unwrap();
    let mut router = Router::default();

    router.add_route(Route::get("/echo", echo_handler, ComparePath::Prefix));
    router.add_route(Route::get("/", ok_handler, ComparePath::Exact));
    router.add_route(Route::get(
        "/user-agent",
        user_agent_handler,
        ComparePath::Exact,
    ));
    router.add_route(Route::get("/files", get_file_handler, ComparePath::Prefix));
    router.add_route(Route::post(
        "/files",
        post_file_handler,
        ComparePath::Prefix,
    ));

    while let Ok((mut stream, _)) = listener.accept().await {
        let router = router.clone();
        tokio::spawn(async move {
            let req = read_stream(&mut stream).await;
            let res = router.route(req);
            write_stream(&mut stream, &res.into_bytes()).await;
        });
    }
}

fn ok_handler(_req: Request) -> Response {
    Response::from(HttpCode::Ok)
}

fn echo_handler(req: Request) -> Response {
    let response_content = req.path().strip_prefix("/echo/").unwrap_or_default();

    let mut response = Response::from(HttpCode::Ok);
    response.header("Content-Type", "text/plain");
    response.header("Content-Length", response_content.len().to_string());
    *response.content_mut() = response_content.into();

    response
}

fn user_agent_handler(req: Request) -> Response {
    let default_user_agent = "No User-Agent".to_string();
    let user_agent = req
        .headers()
        .get("User-Agent")
        .unwrap_or(&default_user_agent)
        .clone();

    let mut response = Response::from(HttpCode::Ok);
    response.header("Content-Type", "text/plain");
    response.header("Content-Length", user_agent.len().to_string());
    *response.content_mut() = user_agent.into_bytes();

    response
}

fn get_file_handler(req: Request) -> Response {
    let dir = std::env::args().nth(2).unwrap();
    let path = req.path().strip_prefix("/files/").unwrap_or_default();
    let file_path = Path::new(&dir);
    let file_path = file_path.join(path);

    if file_path.metadata().is_err() {
        Response::from(HttpCode::NotFound)
    } else {
        let mut file = std::fs::File::open(&file_path).unwrap();
        let mut content = Vec::new();
        file.read_to_end(&mut content).unwrap();

        // Respond with application/octet-stream
        let mut response = Response::from(HttpCode::Ok);
        response.header("Content-Type", "application/octet-stream");
        response.header(
            "Content-Length",
            file_path.metadata().unwrap().len().to_string(),
        );
        *response.content_mut() = content;
        response
    }
}

fn post_file_handler(req: Request) -> Response {
    let dir = std::env::args().nth(2).unwrap();
    let path = req.path().strip_prefix("/files/").unwrap_or_default();
    let file_path = Path::new(&dir);
    let file_path = file_path.join(path);

    let mut file = std::fs::File::create(file_path).unwrap();

    if file.write_all(req.body()).is_err() {
        Response::from(HttpCode::InternalServerError)
    } else {
        Response::from(HttpCode::Created)
    }
}

async fn read_stream(stream: &mut TcpStream) -> Request {
    let mut buf = [0; MAX_BUFFER_SIZE];

    match stream.read(&mut buf).await {
        Ok(_) => Request::parse::<std::array::IntoIter<u8, 2048>>(&mut RequestBuffer::from(
            buf.into_iter(),
        )),
        Err(e) => {
            panic!("Failed to receive data: {}", e);
        }
    }
}

async fn write_stream(stream: &mut TcpStream, data: &[u8]) {
    match stream.write(data).await {
        Ok(_) => {}
        Err(e) => {
            println!("Failed to send data: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    #[test]
    fn test_send_sync() {
        assert_send::<Router>();
        assert_sync::<Router>();
    }
}
