//! Connection boundaries and downstream client identity shared by all adapters.
use http::HeaderMap;

pub(crate) fn transport_headers(headers: &HeaderMap) -> HeaderMap {
    let mut clean = headers.clone();
    for value in headers.get_all("connection") {
        if let Ok(value) = value.to_str() {
            for name in value.split(',') {
                clean.remove(name.trim());
            }
        }
    }
    for name in [
        "connection",
        "keep-alive",
        "proxy-authenticate",
        "proxy-authorization",
        "proxy-connection",
        "te",
        "trailer",
        "transfer-encoding",
        "upgrade",
    ] {
        clean.remove(name);
    }
    clean
}

pub(crate) fn client_header(name: &str) -> bool {
    matches!(
        name,
        "user-agent"
            | "originator"
            | "origin"
            | "referer"
            | "forwarded"
            | "via"
            | "x-real-ip"
            | "true-client-ip"
            | "cf-connecting-ip"
            | "cf-ipcountry"
            | "cf-ray"
            | "cf-visitor"
            | "accept-language"
            | "accept-encoding"
            | "dnt"
            | "priority"
            | "sec-gpc"
            | "upgrade-insecure-requests"
            | "x-requested-with"
            | "x-client-data"
            | "x-openai-client-user-agent"
            | "x-api-key"
            | "api-key"
            | "x-codex-gateway-authorization"
            | "sec-websocket-key"
            | "sec-websocket-version"
            | "sec-websocket-accept"
            | "sec-websocket-extensions"
    ) || name.starts_with("x-stainless-")
        || name.starts_with("sec-ch-")
        || name.starts_with("sec-fetch-")
        || name.starts_with("x-forwarded-")
        || name.starts_with("x-gateway-")
}

pub(crate) fn request_headers(headers: &HeaderMap) -> HeaderMap {
    let mut clean = transport_headers(headers);
    let remove: Vec<_> = clean
        .keys()
        .filter(|name| client_header(name.as_str()))
        .cloned()
        .collect();
    for name in remove {
        clean.remove(name);
    }
    clean
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_policy_keeps_only_explicit_upstream_credentials() {
        use crate::native::{AuthPolicy, request_headers};
        let mut headers = HeaderMap::new();
        for name in [
            "authorization",
            "cookie",
            "chatgpt-account-id",
            "openai-project",
            "openai-organization",
            "x-openai-actor-authorization",
            "x-codex-gateway-authorization",
            "x-api-key",
        ] {
            headers.insert(name, "credential".parse().unwrap());
        }
        for policy in [AuthPolicy::Subscription, AuthPolicy::None] {
            assert!(request_headers(&headers, policy).is_empty());
        }
        let explicit = request_headers(&headers, AuthPolicy::Passthrough);
        assert_eq!(explicit.len(), 6);
        assert_eq!(explicit["authorization"], "credential");
        assert_eq!(explicit["cookie"], "credential");
        assert_eq!(explicit["openai-project"], "credential");
        assert!(!explicit.contains_key("x-codex-gateway-authorization"));
        assert!(!explicit.contains_key("x-api-key"));
    }

    #[test]
    fn protocol_allowlists_honor_connection_tokens_in_both_directions() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "connection",
            "X-Codex-Turn-State, X-Request-Id".parse().unwrap(),
        );
        headers.insert("x-codex-turn-state", "connection-only".parse().unwrap());
        headers.insert("x-request-id", "connection-only".parse().unwrap());
        headers.insert(
            "openai-beta",
            "responses_websockets=2026-02-06".parse().unwrap(),
        );
        headers.insert("retry-after", "7".parse().unwrap());
        let request = crate::server::forward_request_headers(&headers);
        assert_eq!(request.len(), 1);
        assert!(request.contains_key("openai-beta"));
        let response = crate::server::forward_response_headers(&headers);
        assert_eq!(response.len(), 1);
        assert_eq!(response["retry-after"], "7");
    }

    #[test]
    fn strips_all_connection_tokens_and_client_metadata_but_keeps_business_headers() {
        let mut headers = HeaderMap::new();
        headers.append("connection", "X-Request-Id, keep-alive".parse().unwrap());
        headers.append("connection", "mcp-session-id".parse().unwrap());
        for name in [
            "x-request-id",
            "mcp-session-id",
            "user-agent",
            "originator",
            "x-stainless-retry-count",
            "sec-ch-ua",
            "sec-fetch-site",
            "forwarded",
            "x-forwarded-for",
            "x-real-ip",
            "x-gateway-api-key",
            "sec-websocket-key",
            "origin",
            "referer",
            "proxy-connection",
        ] {
            headers.insert(name, "downstream".parse().unwrap());
        }
        for name in [
            "authorization",
            "cookie",
            "openai-project",
            "content-type",
            "range",
            "if-none-match",
            "x-ms-blob-type",
            "openai-beta",
            "session-id",
            "x-codex-turn-state",
            "sec-websocket-protocol",
            "x-future-field",
        ] {
            headers.insert(name, "preserved".parse().unwrap());
        }
        let clean = request_headers(&headers);
        assert!(clean.values().all(|v| v == "preserved"));
        assert_eq!(clean.len(), 12);
        assert_eq!(headers.get_all("connection").iter().count(), 2);
    }
}
