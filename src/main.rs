use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

const MAX_BUFFER_SIZE: usize = 2048;

#[derive(Default)]
struct Router {
    routes: HashMap<String, Box<dyn Fn(Request) -> Response>>,
}

impl Router {
    fn add_route<S, F>(&mut self, path: S, handler: F)
    where
        S: Into<String>,
        F: Fn(Request) -> Response + 'static,
    {
        self.routes.insert(path.into(), Box::new(handler));
    }

    fn route(&self, req: Request) -> Response {
        let mut response = Response::from("HTTP/1.1 404 Not Found\r\n\r\n");
        for (path, handler) in self.routes.iter() {
            if req.path.starts_with(path) {
                response = handler(req);
                break;
            }
        }
        response
    }
}

struct Response {
    content: String,
}

impl AsRef<[u8]> for Response {
    fn as_ref(&self) -> &[u8] {
        self.content.as_bytes()
    }
}

impl<S> From<S> for Response
where
    S: Into<String>,
{
    fn from(value: S) -> Self {
        Response {
            content: value.into(),
        }
    }
}

struct RequestBuffer {
    buffer: Vec<u8>,
    ptr: usize,
}

impl RequestBuffer {
    fn read_until(&mut self, stop: u8, buf: &mut [u8]) -> usize {
        let mut i = 0;
        while self.ptr < self.buffer.len() {
            let byte = self.buffer[self.ptr];
            self.ptr += 1;
            if byte == stop {
                break;
            }
            buf[i] = byte;
            i += 1;
        }
        i
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
}

impl Request {
    fn parse(req_buf: &mut RequestBuffer) -> Request {
        let (method, path, version) = Self::parse_start_line(req_buf);

        Request {
            method,
            path,
            version,
        }
    }

    fn parse_start_line(req_buf: &mut RequestBuffer) -> (Method, String, HttpVersion) {
        let mut buf = Vec::with_capacity(MAX_BUFFER_SIZE);

        req_buf.read_until(b' ', &mut buf);
        let method = Method::from(std::str::from_utf8(&buf).unwrap());
        buf.clear();

        req_buf.read_until(b' ', &mut buf);
        // Safe to unwrap because the buffer is guaranteed to be valid UTF-8
        let path = unsafe { String::from_utf8_unchecked(buf.to_vec()) };
        buf.clear();

        req_buf.read_until(b'\r', &mut buf);
        let version = HttpVersion::from(std::str::from_utf8(&buf).unwrap());

        (method, path, version)
    }
}

fn main() {
    // You can use print statements as follows for debugging, they'll be visible when running tests.
    println!("Logs from your program will appear here!");

    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();
    let mut router = Router::default();

    router.add_route("/", ok_handler);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let req = read_stream(&mut stream.try_clone().unwrap());
                let res = router.route(req);
                write_stream(&mut stream.try_clone().unwrap(), res.as_ref());
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}

fn ok_handler(_req: Request) -> Response {
    Response::from("HTTP/1.1 200 OK\r\n\r\n")
}

fn read_stream(stream: &mut TcpStream) -> Request {
    let mut buf = Vec::new();

    match stream.read_to_end(&mut buf) {
        Ok(_) => {
            let mut req_buf = RequestBuffer {
                buffer: buf,
                ptr: 0,
            };
            Request::parse(&mut req_buf)
        }
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
