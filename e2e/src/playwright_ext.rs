use anyhow::Result;
use async_trait::async_trait;
use futures::StreamExt;
use playwright::api::{
    page::{Event, Page},
    response::Response,
};
use serde::Serialize;
use std::{
    error::Error,
    fmt::{Display, Formatter},
};

#[async_trait]
pub trait PageFetchExt {
    async fn fetch<S: Serialize + Send>(
        &self,
        method: Method,
        url: &str,
        body: Option<S>,
    ) -> Result<Response>;

    async fn get(&self, url: &str) -> Result<Response> {
        self.fetch::<()>(Method::GET, url, None).await
    }

    async fn post<S: Serialize + Send>(&self, url: &str, body: S) -> Result<Response> {
        self.fetch(Method::POST, url, Some(body)).await
    }

    async fn put<S: Serialize + Send>(&self, url: &str, body: S) -> Result<Response> {
        self.fetch(Method::PUT, url, Some(body)).await
    }

    async fn patch<S: Serialize + Send>(&self, url: &str, body: S) -> Result<Response> {
        self.fetch(Method::PATCH, url, Some(body)).await
    }

    async fn delete(&self, url: &str) -> Result<Response> {
        self.fetch::<()>(Method::DELETE, url, None).await
    }
}

#[async_trait]
impl PageFetchExt for Page {
    async fn fetch<S: Serialize + Send>(
        &self,
        method: Method,
        url: &str,
        body: Option<S>,
    ) -> Result<Response> {
        let e2e_fetch_id = self
            .eval::<u32>(r#"() => window.e2eFetchId = (window.e2eFetchId ?? 0) + 1"#)
            .await?;

        // We subscribe to the event stream before calling fetch in order to not miss our response
        let mut response_stream = Box::pin(self.subscribe_event()?.filter_map(
            move |event_result| async move {
                match event_result {
                    Err(err) => Some(Err(err)), // Forward errors
                    Ok(Event::Response(response))
                        if response.request().headers().ok().and_then(|map| {
                            map.get("x-e2e-fetch-id")
                                .and_then(|id| id.parse::<u32>().ok())
                        }) == Some(e2e_fetch_id) =>
                    {
                        Some(Ok(response))
                    }
                    Ok(_) => None, // Ignore other events
                }
            },
        ));

        self.evaluate::<_, ()>(
            r#"([method, url, body, e2eFetchId]) => {
            fetch(url, { method, body: body !== null ? JSON.stringify(body) : null, headers: new Headers({ "x-e2e-fetch-id": e2eFetchId, "Content-Type": "application/json" }) });
        }"#,
            (method.as_str(), url, body, e2e_fetch_id),
        )
        .await?;

        Ok(response_stream.next().await.ok_or(NotFound)??)
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Method {
    GET,
    POST,
    PUT,
    PATCH,
    DELETE,
}

impl Method {
    fn as_str(&self) -> &'static str {
        match self {
            Method::GET => "GET",
            Method::POST => "POST",
            Method::PUT => "PUT",
            Method::PATCH => "PATCH",
            Method::DELETE => "DELETE",
        }
    }
}

#[derive(Debug, Copy, Clone)]
struct NotFound;

impl Display for NotFound {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "NotFound")
    }
}

impl Error for NotFound {}
