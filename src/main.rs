use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

const MAX_BUFFER_SIZE: usize = 2048;

fn main() {
    // You can use print statements as follows for debugging, they'll be visible when running tests.
    println!("Logs from your program will appear here!");

    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                read_stream(&mut stream.try_clone().unwrap());
                write_stream(
                    &mut stream.try_clone().unwrap(),
                    b"HTTP/1.1 200 OK\r\n\r\nHello, world!",
                );
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}

fn read_stream(stream: &mut TcpStream) {
    let mut buffer = [0; MAX_BUFFER_SIZE];
    match stream.read(&mut buffer) {
        Ok(_) => {}
        Err(e) => {
            println!("Failed to receive data: {}", e);
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
