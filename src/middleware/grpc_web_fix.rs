/// Middleware to fix gRPC-Web "trailers-only" responses for Cloudflare Containers.
///
/// When tonic returns a gRPC error, tonic_web produces a response with an empty body
/// and grpc-status/grpc-message in HTTP headers. Cloudflare Containers' container.fetch()
/// crashes on this pattern. This middleware moves the status info into a gRPC-Web trailer
/// frame in the response body.
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::{BufMut, Bytes, BytesMut};
use http::header::HeaderValue;
use http::Request as HttpRequest;
use http::Response as HttpResponse;
use http_body_util::combinators::UnsyncBoxBody;
use http_body_util::{BodyExt, Full};
use tonic::Status;
use tower::{Layer, Service};

type BoxBody = UnsyncBoxBody<Bytes, Status>;

const GRPC_WEB_TRAILERS_BIT: u8 = 0x80;

#[derive(Debug, Clone, Default)]
pub struct GrpcWebTrailerFixLayer;

impl GrpcWebTrailerFixLayer {
    pub fn new() -> Self {
        Self
    }
}

impl<S> Layer<S> for GrpcWebTrailerFixLayer {
    type Service = GrpcWebTrailerFix<S>;

    fn layer(&self, inner: S) -> Self::Service {
        GrpcWebTrailerFix { inner }
    }
}

#[derive(Debug, Clone)]
pub struct GrpcWebTrailerFix<S> {
    inner: S,
}

impl<S, ReqBody> Service<HttpRequest<ReqBody>> for GrpcWebTrailerFix<S>
where
    S: Service<HttpRequest<ReqBody>, Response = HttpResponse<BoxBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
    ReqBody: Send + 'static,
{
    type Response = HttpResponse<BoxBody>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: HttpRequest<ReqBody>) -> Self::Future {
        let mut inner = self.inner.clone();
        std::mem::swap(&mut self.inner, &mut inner);

        Box::pin(async move {
            let response = inner.call(req).await?;

            if !is_grpc_web_trailers_only(&response) {
                return Ok(response);
            }

            Ok(convert_trailers_only_to_body(response))
        })
    }
}

fn is_grpc_web_trailers_only(response: &HttpResponse<BoxBody>) -> bool {
    let headers = response.headers();

    if !headers.contains_key("grpc-status") {
        return false;
    }

    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    content_type.starts_with("application/grpc-web")
}

fn convert_trailers_only_to_body(response: HttpResponse<BoxBody>) -> HttpResponse<BoxBody> {
    let (mut parts, _old_body) = response.into_parts();

    let mut trailer_data = BytesMut::new();

    if let Some(status) = parts.headers.remove("grpc-status") {
        trailer_data.put_slice(b"grpc-status: ");
        trailer_data.put_slice(status.as_bytes());
        trailer_data.put_slice(b"\r\n");
    }

    if let Some(message) = parts.headers.remove("grpc-message") {
        trailer_data.put_slice(b"grpc-message: ");
        trailer_data.put_slice(message.as_bytes());
        trailer_data.put_slice(b"\r\n");
    }

    if let Some(details) = parts.headers.remove("grpc-status-details-bin") {
        trailer_data.put_slice(b"grpc-status-details-bin: ");
        trailer_data.put_slice(details.as_bytes());
        trailer_data.put_slice(b"\r\n");
    }

    let trailer_len = trailer_data.len();
    // Trailer-only frame: connect-web correctly handles 0x80 frame without a preceding
    // data frame. An empty data frame (0x00, length=0) causes connect-web to attempt
    // protobuf deserialization of zero bytes → "incomplete envelope" error.
    let mut frame = BytesMut::with_capacity(5 + trailer_len);
    // Trailer frame: flag=0x80, length=N
    frame.put_u8(GRPC_WEB_TRAILERS_BIT);
    frame.put_u32(trailer_len as u32);
    frame.put(trailer_data);

    let frame_bytes: Bytes = frame.freeze();
    let frame_len = frame_bytes.len();

    let new_body: BoxBody =
        UnsyncBoxBody::new(Full::new(frame_bytes).map_err(|err| match err {}));

    parts.headers.insert(
        "content-length",
        HeaderValue::from_str(&frame_len.to_string()).unwrap(),
    );

    HttpResponse::from_parts(parts, new_body)
}
