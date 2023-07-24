
use crate::types::ResT;
use hyper::{header, Body, Method, Request, Response, Server, StatusCode};
use serde_json::Value;

pub fn build_response(
    body_text: String,
    status_code: StatusCode,
    content_type: &str,
) -> anyhow::Result<ResT> {
    let mut r = Response::builder()
        .status(status_code)
        .body(Body::from(body_text))?;
    r.headers_mut()
        .insert(header::CONTENT_TYPE, content_type.parse()?);
    Ok(r)
}

pub fn response_json(body_text: String, status_code: StatusCode) -> anyhow::Result<ResT> {
    build_response(body_text, status_code, "application/json")
}

pub fn ok_json(v: Value) -> anyhow::Result<ResT> {
    ret_json(StatusCode::OK, v)
}

pub fn ret_json(status_code: StatusCode, v: Value) -> anyhow::Result<ResT> {
    response_json(v.to_string(), status_code)
}

pub fn internal_server_error_resp(error: anyhow::Error) -> anyhow::Result<ResT> {
    build_response(
        format!("Error: {}", error.to_string()),
        StatusCode::INTERNAL_SERVER_ERROR,
        "text/html",
    )
}

pub fn internal_server_error_resp_force(error: anyhow::Error) -> ResT {
    return internal_server_error_resp(error).unwrap();
}
