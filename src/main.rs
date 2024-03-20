use std::collections::HashMap;
use std::io::Read;
use std::iter::Peekable;
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const MAX_BUFFER_SIZE: usize = 2048;

#[derive(Default, Clone)]
struct Router {
    routes: Vec<Route>,
}

type Handler = fn(Request) -> Response;

#[derive(Clone)]
struct Route {
    path: String,
    handler: Box<Handler>,
    compair_method: CompareMethod,
}

impl Route {
    fn new<S>(path: S, handler: Handler, compair_method: CompareMethod) -> Self
    where
        S: Into<String>,
    {
        Route {
            path: path.into(),
            handler: Box::new(handler),
            compair_method,
        }
    }

    fn matches(&self, req: &Request) -> bool {
        match self.compair_method {
            CompareMethod::Exact => self.path == req.path,
            CompareMethod::Prefix => req.path.starts_with(&self.path),
        }
    }
}

#[derive(Clone, Copy)]
enum CompareMethod {
    Exact,
    Prefix,
}

impl Router {
    fn add_route(&mut self, route: Route) {
        self.routes.push(route);
    }

    fn route(&self, req: Request) -> Response {
        let mut response = Response::from(HttpCode::NotFound);

        if let Some(route) = self.routes.iter().find(|route| route.matches(&req)) {
            response = (route.handler)(req);
        }

        response
    }
}

#[derive(Clone, Copy)]
enum HttpCode {
    Ok = 200,
    NotFound = 404,
}

impl std::fmt::Display for HttpCode {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use HttpCode::*;

        match self {
            Ok => write!(f, "200 OK"),
            NotFound => write!(f, "404 Not Found"),
        }
    }
}

#[derive(Clone)]
struct Response {
    code: HttpCode,
    content: Vec<u8>,
    headers: HashMap<String, String>,
}

impl Response {
    fn header<K, V>(&mut self, key: K, value: V)
    where
        K: Into<String>,
        V: Into<String>,
    {
        self.headers.insert(key.into(), value.into());
    }

    fn into_bytes(mut self) -> Vec<u8> {
        let mut buf = format!("HTTP/1.1 {}\r\n", self.code).into_bytes();
        for (key, value) in self.headers {
            let mut header = format!("{}: {}\r\n", key, value).into_bytes();
            buf.append(&mut header);
        }
        buf.append(&mut b"\r\n".to_vec());
        buf.append(&mut self.content);
        buf
    }
}

impl From<HttpCode> for Response {
    fn from(code: HttpCode) -> Self {
        Response {
            code,
            content: Vec::new(),
            headers: HashMap::new(),
        }
    }
}

impl<C> From<C> for Response
where
    C: Into<Vec<u8>>,
{
    fn from(value: C) -> Self {
        Response {
            code: HttpCode::Ok,
            content: value.into(),
            headers: HashMap::new(),
        }
    }
}

struct RequestBuffer<I>
where
    I: Iterator<Item = u8>,
{
    iter: Peekable<I>,
}

impl<I> RequestBuffer<I>
where
    I: Iterator<Item = u8>,
{
    fn read_until(&mut self, stop: u8, buf: &mut Vec<u8>) -> usize {
        let mut i = 0;
        while let Some(byte) = self.iter.peek() {
            if *byte == stop {
                break;
            }
            buf.push(*byte);
            self.iter.next();
            i += 1;
        }
        i
    }

    fn read_next_line(&mut self, buf: &mut Vec<u8>) -> usize {
        let mut i = 0;
        let mut last_byte = 0;
        while let Some(&byte) = self.iter.peek() {
            if byte == b'\n' && last_byte == b'\r' {
                buf.pop();
                // Consume the \n
                self.iter.next();
                break;
            }
            buf.push(byte);
            self.iter.next();
            last_byte = byte;
            i += 1;
        }
        i
    }
}

impl<I> From<I> for RequestBuffer<I>
where
    I: Iterator<Item = u8>,
{
    fn from(iter: I) -> Self {
        RequestBuffer {
            iter: iter.peekable(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Method {
    GET,
    POST,
    PUT,
    DELETE,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HttpVersion {
    V1_0,
    V1_1,
}

impl<S> From<S> for Method
where
    S: AsRef<str>,
{
    fn from(value: S) -> Self {
        match value.as_ref() {
            "GET" => Method::GET,
            "POST" => Method::POST,
            "PUT" => Method::PUT,
            "DELETE" => Method::DELETE,
            _ => panic!("Invalid method"),
        }
    }
}

impl<S> From<S> for HttpVersion
where
    S: AsRef<str>,
{
    fn from(value: S) -> Self {
        match value.as_ref() {
            "HTTP/1.0" => HttpVersion::V1_0,
            "HTTP/1.1" => HttpVersion::V1_1,
            _ => panic!("Invalid HTTP version"),
        }
    }
}

#[derive(Debug, Clone)]
struct Request {
    method: Method,
    path: String,
    version: HttpVersion,
    headers: HashMap<String, String>,
}

impl Request {
    fn parse<I>(req_buf: &mut RequestBuffer<I>) -> Request
    where
        I: Iterator<Item = u8>,
    {
        let (method, path, version) = Self::parse_start_line(req_buf);
        let headers = Self::parse_headers(req_buf);

        Request {
            method,
            path,
            version,
            headers,
        }
    }

    fn parse_start_line<I>(req_buf: &mut RequestBuffer<I>) -> (Method, String, HttpVersion)
    where
        I: Iterator<Item = u8>,
    {
        let mut buf = Vec::new();
        req_buf.read_next_line(&mut buf);

        let parts = buf.split(|&c| c == b' ').collect::<Vec<_>>();
        assert_eq!(parts.len(), 3);

        let method = Method::from(std::str::from_utf8(parts[0]).unwrap());
        let path = unsafe { String::from_utf8_unchecked(parts[1].to_vec()) };
        let version = HttpVersion::from(std::str::from_utf8(parts[2]).unwrap());

        (method, path, version)
    }

    fn parse_headers<I>(req_buf: &mut RequestBuffer<I>) -> HashMap<String, String>
    where
        I: Iterator<Item = u8>,
    {
        let mut headers = HashMap::new();
        let mut buf = Vec::new();
        while req_buf.read_next_line(&mut buf) > 0 && buf.len() > 2 {
            let parts = buf.split(|&b| b == b':').collect::<Vec<_>>();
            assert!(parts.len() >= 2);

            let key = parts[0];
            let value = parts[1..].concat();

            let key = unsafe { std::str::from_utf8_unchecked(key).trim().to_string() };
            let value = unsafe { std::str::from_utf8_unchecked(&value).trim().to_string() };
            headers.insert(key, value);
            buf.clear();
        }
        headers
    }
}

#[tokio::main]
async fn main() {
    // You can use print statements as follows for debugging, they'll be visible when running tests.
    println!("Logs from your program will appear here!");

    let listener = TcpListener::bind("127.0.0.1:4221").await.unwrap();
    let mut router = Router::default();

    router.add_route(Route::new("/echo", echo_handler, CompareMethod::Prefix));
    router.add_route(Route::new("/", ok_handler, CompareMethod::Exact));
    router.add_route(Route::new(
        "/user-agent",
        user_agent_handler,
        CompareMethod::Exact,
    ));
    router.add_route(Route::new(
        "/files",
        get_file_handler,
        CompareMethod::Prefix,
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
    let response_content = req.path.strip_prefix("/echo/").unwrap_or_default();

    let mut response = Response::from(HttpCode::Ok);
    response.header("Content-Type", "text/plain");
    response.header("Content-Length", response_content.len().to_string());
    response.content = response_content.into();

    response
}

fn user_agent_handler(req: Request) -> Response {
    let default_user_agent = "No User-Agent".to_string();
    let user_agent = req
        .headers
        .get("User-Agent")
        .unwrap_or(&default_user_agent)
        .clone();

    let mut response = Response::from(HttpCode::Ok);
    response.header("Content-Type", "text/plain");
    response.header("Content-Length", user_agent.len().to_string());
    response.content = user_agent.into_bytes();

    response
}

fn get_file_handler(req: Request) -> Response {
    let dir = std::env::args().nth(2).unwrap();
    let path = req.path.strip_prefix("/files/").unwrap_or_default();
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
        response.content = content;
        response
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
