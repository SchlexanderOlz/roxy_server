use std::net::TcpStream;

pub struct ConnectionRequest<'a> {
    pub method: &'a str,
    pub destination: &'a str,
    pub http_version: &'a str,
    pub tcp_stream: &'a mut TcpStream,
}
