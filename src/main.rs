use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
};

use reqwest::{self, Response};
use tokio;

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

    let listener = TcpListener::bind(addr)?;
    for stream in listener.incoming() {
        let _ = tokio::task::spawn(handle_client(stream?)).await;
        return Ok(());
    }
    Ok(())
}

async fn handle_client(mut stream: TcpStream) -> std::io::Result<()> {
    let mut buff = [0u8; 1024];
    while let Ok(bytes_read) = stream.read(&mut buff) {
        if bytes_read == 0 {
            return Ok(());
        }
        let message = std::str::from_utf8(&buff[0..bytes_read]).unwrap();
        println!("Received data {}", message);
        let mut headers = message.split("\r");

        let request_info = headers.next().unwrap().split(" ");
        let destination = request_info.skip(1).next().unwrap();

        let response = match reqwest::get(destination).await {
            Ok(response) => response,
            Err(_) => {
                println!("Couldnt get {}", destination);
                continue;
            }
        };

        let response_string = parse_response(response).await;

        let _ = stream.write(response_string.as_bytes());
        println!("{}", response_string);

        buff = [0u8; 1024];
    }
    Ok(())
}

async fn parse_response(response: Response) -> String {
    let mut response_string = format!(
        "HTTP/1.1 {} {}\r\n",
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
