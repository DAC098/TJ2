use axum::body::Body;
use axum::http::StatusCode;
use axum::response::Response;
use bytes::{BufMut, Bytes, BytesMut};
use serde::Serialize;

pub fn json_bytes(data: &impl Serialize) -> Result<Bytes, serde_json::Error> {
    let mut buf = BytesMut::with_capacity(128).writer();
    serde_json::to_writer(&mut buf, data)?;

    Ok(buf.into_inner().freeze())
}

pub fn serialize_json(
    status: StatusCode,
    data: &impl Serialize,
) -> Result<Response, serde_json::Error> {
    json_bytes(data).map(|buf| {
        Response::builder()
            .status(status)
            .header("content-type", "application/json; charset=utf-8")
            .header("content-length", buf.len())
            .body(Body::from(buf))
            .unwrap()
    })
}
