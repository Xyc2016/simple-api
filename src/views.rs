use anyhow::anyhow;
use async_trait::async_trait;
use hyper::{header, Body, Method, Response, StatusCode};
use regex::Regex;
use tokio::fs::File;
use tokio::io::AsyncReadExt; // for read_to_end()

use crate::{
    context::Context,
    types::{HttpRequest, HttpResonse},
    view::View,
};

pub struct StaticFiles {
    root: String,
    re_path: Regex,
}

impl StaticFiles {
    pub fn new(root: String, re_path: Regex) -> Self {
        Self { root, re_path }
    }
}

#[async_trait]
impl View for StaticFiles {
    fn re_path(&self) -> Regex {
        self.re_path.clone()
    }
    fn methods(&self) -> Vec<Method> {
        vec![Method::GET]
    }
    async fn call(&self, req: &mut HttpRequest, ctx: &mut Context) -> anyhow::Result<HttpResonse> {
        let view_args = &ctx.view_args;
        let file_path = view_args
            .as_ref()
            .ok_or(anyhow!("no view_args"))?
            .get("file_path")
            .ok_or(anyhow!("no file_path"))?;
        let file_path = format!("{}/{}", self.root, file_path);
        let file_path = std::path::Path::new(&file_path);
        if !file_path.exists() {
            return Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("Not found"))?);
        }

        let mut file = tokio::fs::File::open(file_path).await?;
        let mut buf: Vec<u8> = Vec::new();
        file.read_to_end(&mut buf).await?;
        let mime = mime_guess::from_path(file_path).first_or_octet_stream();
        let mut r = Response::builder()
            .status(StatusCode::OK)
            .body(Body::from(buf))?;
        r.headers_mut()
            .insert(header::CONTENT_TYPE, (&mime.to_string()).parse()?);
        Ok(r)
    }
}
