use std::collections::HashMap;
use std::io::{Read, Write};
use std::iter::Peekable;
use std::net::{TcpListener, TcpStream};

use itertools::Itertools;

const MAX_BUFFER_SIZE: usize = 2048;

#[derive(Default)]
struct Router {
    routes: Vec<Route>,
}

struct Route {
    path: String,
    handler: Box<dyn Fn(Request) -> Response>,
    compair_method: CompareMethod,
}

impl Route {
    fn new<S, F>(path: S, handler: F, compair_method: CompareMethod) -> Self
    where
        S: Into<String>,
        F: Fn(Request) -> Response + 'static,
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

struct Response {
    code: HttpCode,
    content: String,
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

    fn into_bytes(self) -> Vec<u8> {
        let mut buf = format!("HTTP/1.1 {}\r\n", self.code);
        for (key, value) in self.headers {
            buf.push_str(&format!("{}: {}\r\n", key, value));
        }
        buf.push_str("\r\n");
        buf.push_str(&self.content);
        buf.into_bytes()
    }
}

impl From<HttpCode> for Response {
    fn from(code: HttpCode) -> Self {
        Response {
            code,
            content: String::new(),
            headers: HashMap::new(),
        }
    }
}

impl<S> From<S> for Response
where
    S: Into<String>,
{
    fn from(value: S) -> Self {
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

fn main() {
    // You can use print statements as follows for debugging, they'll be visible when running tests.
    println!("Logs from your program will appear here!");

    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();
    let mut router = Router::default();

    router.add_route(Route::new("/echo", echo_handler, CompareMethod::Prefix));
    router.add_route(Route::new("/", ok_handler, CompareMethod::Exact));
    router.add_route(Route::new(
        "/user-agent",
        user_agent_handler,
        CompareMethod::Exact,
    ));

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let req = read_stream(&mut stream);
                let res = router.route(req);
                write_stream(&mut stream, &res.into_bytes());
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
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
    response.content = response_content.to_string();

    response
}

fn user_agent_handler(req: Request) -> Response {
    let default_user_agent = "No User-Agent".to_string();
    let user_agent = req.headers.get("User-Agent").unwrap_or(&default_user_agent);

    let mut response = Response::from(HttpCode::Ok);
    response.header("Content-Type", "text/plain");
    response.header("Content-Length", user_agent.len().to_string());
    response.content = user_agent.to_string();

    response
}

fn read_stream(stream: &mut TcpStream) -> Request {
    let mut buf = [0; MAX_BUFFER_SIZE];

    match stream.read(&mut buf) {
        Ok(_) => Request::parse::<std::array::IntoIter<u8, 2048>>(&mut RequestBuffer::from(
            buf.into_iter(),
        )),
        Err(e) => {
            panic!("Failed to receive data: {}", e);
        }
    }
}

fn write_stream(stream: &mut TcpStream, data: &[u8]) {
    match stream.write(data) {
        Ok(_) => {}
        Err(e) => {
            println!("Failed to send data: {}", e);
        }
    }
}
