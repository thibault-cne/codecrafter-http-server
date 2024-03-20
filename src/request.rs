#![allow(dead_code)]

use std::collections::HashMap;
use std::iter::Peekable;

use super::{HttpVersion, Method};

#[derive(Debug, Clone)]
pub struct Request {
    method: Method,
    path: String,
    version: HttpVersion,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl Request {
    pub fn method(&self) -> Method {
        self.method
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }

    pub fn parse<I>(req_buf: &mut RequestBuffer<I>) -> Request
    where
        I: Iterator<Item = u8>,
    {
        let (method, path, version) = Self::parse_start_line(req_buf);
        let headers = Self::parse_headers(req_buf);
        let body = Self::parse_body(req_buf);

        Request {
            method,
            path,
            version,
            headers,
            body,
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

    fn parse_body<I>(req_buf: &mut RequestBuffer<I>) -> Vec<u8>
    where
        I: Iterator<Item = u8>,
    {
        let mut body = Vec::new();
        req_buf.read_to_end(&mut body);
        body
    }
}

pub struct RequestBuffer<I>
where
    I: Iterator<Item = u8>,
{
    iter: Peekable<I>,
}

impl<I> RequestBuffer<I>
where
    I: Iterator<Item = u8>,
{
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

    fn read_to_end(&mut self, buf: &mut Vec<u8>) {
        for byte in self.iter.by_ref() {
            if byte == 0 {
                break;
            }
            buf.push(byte);
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_start_line() {
        let mut buf = RequestBuffer::from("GET / HTTP/1.1\r\n".bytes());
        let (method, path, version) = Request::parse_start_line(&mut buf);
        assert_eq!(method, Method::Get);
        assert_eq!(path, "/");
        assert_eq!(version, HttpVersion::V1_1);
    }

    #[test]
    fn test_parse_headers() {
        let mut buf = RequestBuffer::from("Host: localhost\r\nContent-Length: 10\r\n\r\n".bytes());
        let headers = Request::parse_headers(&mut buf);
        assert_eq!(headers.get("Host").unwrap(), "localhost");
        assert_eq!(headers.get("Content-Length").unwrap(), "10");
    }

    #[test]
    fn test_parse_body() {
        let mut buf = RequestBuffer::from("Hello, World!".bytes());
        let body = Request::parse_body(&mut buf);
        assert_eq!(body, "Hello, World!".as_bytes());
    }

    #[test]
    fn test_parse() {
        let mut buf = RequestBuffer::from(
            "GET / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 10\r\n\r\nHello, World!".bytes(),
        );
        let req = Request::parse(&mut buf);
        assert_eq!(req.method(), Method::Get);
        assert_eq!(req.path(), "/");
        assert_eq!(req.headers().get("Host").unwrap(), "localhost");
        assert_eq!(req.headers().get("Content-Length").unwrap(), "10");
        assert_eq!(req.body(), "Hello, World!".as_bytes());
        assert_eq!(req.version, HttpVersion::V1_1);
    }
}
