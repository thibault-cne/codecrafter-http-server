use std::collections::HashMap;

use super::HttpCode;

#[derive(Clone)]
pub struct Response {
    code: HttpCode,
    content: Vec<u8>,
    headers: HashMap<String, String>,
}

impl Response {
    pub fn content_mut(&mut self) -> &mut Vec<u8> {
        &mut self.content
    }

    pub fn header<K, V>(&mut self, key: K, value: V)
    where
        K: Into<String>,
        V: Into<String>,
    {
        self.headers.insert(key.into(), value.into());
    }

    pub fn into_bytes(mut self) -> Vec<u8> {
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
