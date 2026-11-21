use gpui::BackgroundExecutor;
use gpui::http_client::{
    AsyncBody, HttpClient, Method, Request, Response, Result, anyhow, http::HeaderValue,
};
use std::{future::Future, pin::Pin, process::Command};

/// GPUI's image loader handles decoding and caching. Supply its HTTP transport
/// using the system curl, keeping the desktop example independent of a runtime.
pub(super) struct ImageClient(pub BackgroundExecutor);

impl HttpClient for ImageClient {
    fn proxy(&self) -> Option<&gpui::http_client::Url> {
        None
    }
    fn type_name(&self) -> &'static str {
        "image fetcher"
    }
    fn user_agent(&self) -> Option<&HeaderValue> {
        None
    }

    fn send(
        &self,
        request: Request<AsyncBody>,
    ) -> Pin<Box<dyn Future<Output = Result<Response<AsyncBody>>> + Send>> {
        let task = self.0.spawn(async move {
            if request.method() != Method::GET {
                return Err(anyhow!("expected an image GET request"));
            }
            let output = Command::new("curl")
                .args([
                    "--disable",
                    "--globoff",
                    "--fail",
                    "--silent",
                    "--show-error",
                    "--location",
                    "--proto",
                    "=http,https",
                    "--proto-redir",
                    "=http,https",
                    "--max-time",
                    "20",
                    "--max-filesize",
                    "16777216",
                    "--",
                    &request.uri().to_string(),
                ])
                .output()?;
            if !output.status.success() {
                return Err(anyhow!("image download failed: {}", output.status));
            }
            Ok(Response::new(output.stdout.into()))
        });
        Box::pin(task)
    }
}
