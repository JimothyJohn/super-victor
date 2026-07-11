//! SSE integration: real TCP against a served router, since the stream never
//! terminates and buffered test clients would hang on it. All reads are
//! wrapped in timeouts — a hang is a failure, not a stall.

mod common;

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

use supervictor_endpoint::routes;

const ADMIN_DN: &str = "CN=nick,OU=admin,O=supervictor";
const IO_TIMEOUT: Duration = Duration::from_secs(5);

/// Serve the app on an ephemeral port, return its address.
async fn serve() -> String {
    let store = common::test_store();
    let app = routes::router(store);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("127.0.0.1:{}", addr.port())
}

async fn open_sse(addr: &str) -> TcpStream {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "GET /ui/events HTTP/1.1\r\nHost: {addr}\r\nx-ssl-client-subject-dn: {ADMIN_DN}\r\nAccept: text/event-stream\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    stream
}

/// Read from `stream` until `needle` appears or the timeout hits.
async fn read_until(stream: &mut TcpStream, needle: &str) -> String {
    let mut collected = Vec::new();
    let mut buf = [0u8; 1024];
    let deadline = tokio::time::Instant::now() + IO_TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let n = timeout(remaining, stream.read(&mut buf))
            .await
            .unwrap_or_else(|_| panic!("timed out waiting for {needle:?}"))
            .unwrap();
        assert!(n > 0, "connection closed while waiting for {needle:?}");
        collected.extend_from_slice(&buf[..n]);
        let text = String::from_utf8_lossy(&collected).to_string();
        if text.contains(needle) {
            return text;
        }
    }
}

async fn post_uplink(addr: &str, device_id: &str, current: i32) -> String {
    let body = format!(r#"{{"id":"{device_id}","current":{current}}}"#);
    let req = format!(
        "POST / HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut response = String::new();
    timeout(IO_TIMEOUT, stream.read_to_string(&mut response))
        .await
        .expect("uplink response timed out")
        .unwrap();
    response
}

#[tokio::test]
async fn sse_streams_uplinks_as_they_arrive() {
    let addr = serve().await;

    let mut sse = open_sse(&addr).await;
    let headers = read_until(&mut sse, "\r\n\r\n").await;
    assert!(headers.contains("200 OK"), "headers: {headers}");
    assert!(
        headers.contains("text/event-stream"),
        "wrong content type: {headers}"
    );

    let resp = post_uplink(&addr, "live-device", 77).await;
    assert!(resp.contains("200 OK"), "uplink failed: {resp}");

    let event = read_until(&mut sse, "live-device").await;
    assert!(event.contains("event: uplink"), "got: {event}");
    assert!(event.contains(r#""current":77"#), "got: {event}");
}

#[tokio::test]
async fn sse_requires_admin_certificate() {
    let addr = serve().await;
    let mut stream = TcpStream::connect(&addr).await.unwrap();
    let req = format!("GET /ui/events HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut response = String::new();
    timeout(IO_TIMEOUT, stream.read_to_string(&mut response))
        .await
        .expect("response timed out")
        .unwrap();
    assert!(response.contains("403"), "got: {response}");
}

#[tokio::test]
async fn slow_sse_consumer_does_not_wedge_ingest() {
    let addr = serve().await;

    // Open a subscriber and then never read from it again.
    let mut sse = open_sse(&addr).await;
    let _ = read_until(&mut sse, "\r\n\r\n").await;
    // sse is now parked with an unread kernel buffer.

    // Ingest must keep returning 200 far past every buffer in the chain.
    for i in 0..100 {
        let resp = post_uplink(&addr, "flooder", i).await;
        assert!(
            resp.contains("200 OK"),
            "uplink {i} failed with slow consumer attached"
        );
    }
}
