//! Gateway entry points over the unchanged endpoint authentication and retry machinery.

use crate::auth::SharedAuthProvider;
use crate::endpoint::session::EndpointSession;
use crate::error::ApiError;
use crate::provider::Provider;
use codex_client::{HttpTransport, RequestBody, Response, StreamResponse};
use http::{HeaderMap, Method};

pub struct RawClient<T: HttpTransport> {
    session: EndpointSession<T>,
}

impl<T: HttpTransport> RawClient<T> {
    pub fn new(transport: T, provider: Provider, auth: SharedAuthProvider) -> Self {
        Self {
            session: EndpointSession::new(transport, provider, auth),
        }
    }

    pub async fn execute(
        &self,
        method: Method,
        path: &str,
        headers: HeaderMap,
        body: Option<RequestBody>,
    ) -> Result<Response, ApiError> {
        self.session
            .execute_with(method, path, headers, None, |request| {
                request.body = body.clone();
            })
            .await
    }

    pub async fn stream(
        &self,
        method: Method,
        path: &str,
        headers: HeaderMap,
        body: Option<RequestBody>,
    ) -> Result<StreamResponse, ApiError> {
        self.session
            .stream_encoded_json_with(method, path, headers, None, |request| {
                request.body = body.clone();
            })
            .await
    }
}
