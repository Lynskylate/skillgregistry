use anyhow::Result;
use reqwest::{Client, RequestBuilder, Response, StatusCode};
use std::time::Duration;

const DEFAULT_USER_AGENT: &str = "SkillRegistry/1.0";
const GITHUB_ACCEPT_HEADER: &str = "application/vnd.github.v3+json";

pub fn build_github_client(token: Option<&str>) -> Result<Client> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::USER_AGENT, DEFAULT_USER_AGENT.parse()?);
    headers.insert(reqwest::header::ACCEPT, GITHUB_ACCEPT_HEADER.parse()?);

    if let Some(raw_token) = token {
        let token = raw_token.trim();
        if !token.is_empty() {
            headers.insert(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", token).parse()?,
            );
        }
    }

    Ok(Client::builder().default_headers(headers).build()?)
}

pub async fn send_request_with_retry(req: RequestBuilder, context: &str) -> Result<Response> {
    let mut attempts = 0;
    loop {
        attempts += 1;
        let response = req
            .try_clone()
            .ok_or_else(|| anyhow::anyhow!("failed to clone request"))?
            .send()
            .await?;

        match response.status() {
            StatusCode::OK => return Ok(response),
            StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS => {
                if attempts >= 5 {
                    return Err(anyhow::anyhow!(
                        "Rate limit exceeded after {} attempts ({})",
                        attempts,
                        context
                    ));
                }

                let wait_time = response
                    .headers()
                    .get("Retry-After")
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse::<u64>().ok())
                    .unwrap_or(60);
                tracing::warn!(
                    context,
                    attempts,
                    wait_time,
                    "GitHub rate limit hit, waiting before retry"
                );
                tokio::time::sleep(Duration::from_secs(wait_time)).await;
            }
            StatusCode::UNPROCESSABLE_ENTITY => {
                return Err(anyhow::anyhow!(
                    "GitHub API returned 422 Unprocessable Entity ({})",
                    context
                ));
            }
            status => {
                if attempts >= 3 {
                    return Err(anyhow::anyhow!(
                        "GitHub request failed with status {} ({})",
                        status,
                        context
                    ));
                }

                tokio::time::sleep(Duration::from_secs(2u64.pow(attempts))).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_github_client_accepts_token_and_blank_token() {
        let with_token = build_github_client(Some("token-123")).unwrap();
        let _req = with_token.get("https://example.com").build().unwrap();

        let blank = build_github_client(Some("   ")).unwrap();
        let _req = blank.get("https://example.com").build().unwrap();
    }

    #[tokio::test]
    async fn send_request_with_retry_surfaces_connection_errors() {
        let client = reqwest::Client::new();
        let req = client.get("http://127.0.0.1:1/unreachable");
        let err = send_request_with_retry(req, "test").await.unwrap_err();
        assert!(err.to_string().contains("error"));
    }
}
