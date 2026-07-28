//! Feature-gated, loopback-only replay viewer server.
//!
//! This module is never linked into release artifacts. It intentionally uses
//! only `std::net`, accepts only HTTP GET requests, and binds to 127.0.0.1.

#![forbid(unsafe_code)]

use crate::args::Args;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::time::Duration;

const MAX_REQUEST_BYTES: usize = 8 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(2);

pub fn run(args: &Args) -> Result<(), String> {
    let port = args
        .port
        .ok_or_else(|| "serve-replay requires --port <n>".to_string())?;
    let listener =
        TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).map_err(|error| {
            format!("E-REPLAY-SERVER-001: could not bind 127.0.0.1:{port}: {error}")
        })?;
    let address = listener
        .local_addr()
        .map_err(|error| format!("E-REPLAY-SERVER-002: could not read bound address: {error}"))?;
    println!("serve-replay: listening {address}");

    for incoming in listener.incoming() {
        let mut stream =
            incoming.map_err(|error| format!("E-REPLAY-SERVER-003: accept failed: {error}"))?;
        serve_connection(&mut stream)?;
    }
    Ok(())
}

fn serve_connection(stream: &mut TcpStream) -> Result<(), String> {
    stream
        .set_read_timeout(Some(IO_TIMEOUT))
        .map_err(|error| format!("E-REPLAY-SERVER-004: read timeout failed: {error}"))?;
    stream
        .set_write_timeout(Some(IO_TIMEOUT))
        .map_err(|error| format!("E-REPLAY-SERVER-005: write timeout failed: {error}"))?;

    let mut request = [0_u8; MAX_REQUEST_BYTES];
    let count = stream
        .read(&mut request)
        .map_err(|error| format!("E-REPLAY-SERVER-006: request read failed: {error}"))?;
    let request_line = std::str::from_utf8(&request[..count])
        .ok()
        .and_then(|text| text.lines().next())
        .unwrap_or_default();
    let (status, body, content_type) = if request_line.starts_with("GET /health ") {
        ("200 OK", "replay-server: ok\n", "text/plain; charset=utf-8")
    } else if request_line.starts_with("GET / ") {
        (
            "200 OK",
            "<!doctype html><html><head><meta charset=\"utf-8\"><title>POWDERBURN Replay</title></head><body><main><h1>POWDERBURN Replay Viewer</h1><p>Loopback development server ready.</p></main></body></html>",
            "text/html; charset=utf-8",
        )
    } else {
        ("404 Not Found", "not found\n", "text/plain; charset=utf-8")
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .map_err(|error| format!("E-REPLAY-SERVER-007: response write failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Debug;
    use std::net::TcpStream;
    use std::thread;

    fn must<T, E: Debug>(result: Result<T, E>, operation: &str) -> T {
        match result {
            Ok(value) => value,
            Err(error) => panic!("{operation} failed: {error:?}"),
        }
    }

    #[test]
    fn health_endpoint_is_loopback_only_and_bounded() {
        let listener = must(
            TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)),
            "bind loopback",
        );
        let address = must(listener.local_addr(), "read bound address");
        assert!(address.ip().is_loopback());
        let server = thread::spawn(move || {
            let (mut stream, _) = must(listener.accept(), "accept");
            must(serve_connection(&mut stream), "serve");
        });
        let mut client = must(TcpStream::connect(address), "connect");
        must(
            client.write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n"),
            "write request",
        );
        let mut response = String::new();
        must(client.read_to_string(&mut response), "read response");
        match server.join() {
            Ok(()) => {}
            Err(error) => panic!("server thread failed: {error:?}"),
        }
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(response.ends_with("replay-server: ok\n"));
    }
}
