use crate::connection::ConnectionRequest;
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod, SslStream};
use reqwest::{self, Response};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
};
use tokio;

mod connection;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let mut arg_iter = std::env::args().into_iter().skip(1);

    let mut addr: String = "127.0.0.1:8080".to_string();
    while let Some(arg) = arg_iter.next() {
        match arg.as_str() {
            "--address" => {
                addr = arg_iter
                    .next()
                    .expect("Address was not supplied any value")
                    .clone()
            }
            _ => panic!("Unknow argument {}", arg.as_str()),
        }
    }
    let server = ProxyServer::new();
    let _ = server.listen(addr).await;

    Ok(())
}

struct ProxyServer {
    ssl_acceptor: SslAcceptor,
}

impl ProxyServer {
    pub fn new() -> Self {
        let ssl_acceptor: SslAcceptor = {
            let mut acceptor_builder = SslAcceptor::mozilla_modern(SslMethod::tls()).unwrap();
            acceptor_builder
                .set_certificate_chain_file("keys/cert.pem")
                .unwrap();
            acceptor_builder
                .set_private_key_file("keys/key.pem", SslFiletype::PEM)
                .unwrap();
            acceptor_builder.build()
        };
        Self { ssl_acceptor }
    }

    pub async fn listen(self, addr: String) -> std::io::Result<()> {
        let listener = TcpListener::bind(addr)?;
        for stream in listener.incoming() {
            let _ = tokio::task::spawn(self.handle_client(stream?)).await;
            return Ok(());
        }
        Ok(())
    }

    async fn handle_client(&self, mut stream: TcpStream) -> std::io::Result<()> {
        let mut buff = [0u8; 1024];
        while let Ok(bytes_read) = stream.read(&mut buff) {
            if bytes_read == 0 {
                return Ok(());
            }
            println!("New Connection from {}", stream.peer_addr()?);
            let message = match std::str::from_utf8(&buff[0..bytes_read]) {
                Ok(message) => message,
                Err(_) => {
                    let ssl_stream = self.ssl_acceptor.accept(stream).unwrap();
                    self.handle_encrypted_client(ssl_stream, &buff).await;
                    continue;
                }
            };
            println!("Received data {}", message);
            let mut headers = message.split("\r");

            let mut request_info = headers.next().unwrap().split(" ");
            let method = request_info.next().unwrap().trim();
            let destination = request_info.next().unwrap();
            let http_version = request_info.next().unwrap();

            let connection_request = ConnectionRequest {
                method,
                destination,
                http_version,
                tcp_stream: &mut stream,
            };

            match method {
                "GET" => self.do_get(connection_request).await,
                "CONNECT" => self.handle_https(connection_request).await,
                _ => panic!("sasa"),
            }
            buff = [0u8; 1024];
        }
        Ok(())
    }

    async fn parse_response(response: Response, http_version: &str) -> String {
        let mut response_string = format!(
            "{} {} {}\r\n",
            http_version,
            response.status(),
            response.status().canonical_reason().unwrap_or("")
        );
        for (name, value) in response.headers() {
            response_string.push_str(&format!(
                "{}: {}\r\n",
                name.as_str(),
                value.to_str().unwrap_or("")
            ));
        }
        response_string.push_str("\r\n");
        let body = response.text().await.unwrap_or_default();
        response_string.push_str(&body);
        response_string
    }

    async fn do_get(&self, request: ConnectionRequest<'_>) {
        let response = match reqwest::get(request.destination).await {
            Ok(response) => response,
            Err(_) => {
                println!("Couldnt get {}", request.destination);
                return;
            }
        };

        let response_string = Self::parse_response(response, request.http_version).await;

        let _ = request.tcp_stream.write(response_string.as_bytes());
        println!("{}", response_string);
    }

    async fn handle_https(&self, request: ConnectionRequest<'_>) {
        let client = reqwest::Client::new();
        let connection_established = format!(
            "{} 200 Connection Established\r\n\r\n",
            request.http_version
        );
        let _ = request
            .tcp_stream
            .write_all(connection_established.as_bytes());

        let destination = format!("https://{}", request.destination);
        let response = match reqwest::get(destination).await {
            Ok(response) => response,
            Err(_) => return,
        };
        for (header, value) in response.headers() {
            println!("{} : {}", header.as_str(), value.to_str().unwrap());
        }
        let response_string = Self::parse_response(response, request.http_version).await;
        let _ = request.tcp_stream.write(response_string.as_bytes());
    }

    async fn handle_encrypted_client(&self, stream: SslStream<TcpStream>, buff: &[u8; 1024]) {
        let message = unsafe { std::str::from_utf8_unchecked(buff) };
        println!("{}", message);
    }
}
