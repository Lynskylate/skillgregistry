use anyhow::Result;
use aws_config::meta::region::RegionProviderChain;
use aws_config::BehaviorVersion;
use aws_credential_types::{provider::SharedCredentialsProvider, Credentials};
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart};
use aws_sdk_s3::Client;
use base64::{engine::general_purpose, Engine as _};
use md5;

pub struct S3Service {
    client: Client,
    bucket: String,
    base_url: String,
}

fn normalize_endpoint(endpoint: &str) -> String {
    let trimmed = endpoint.trim_matches('"').trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{}", trimmed)
    }
}

fn endpoint_for_url(endpoint: Option<&str>) -> String {
    endpoint
        .map(normalize_endpoint)
        .unwrap_or_else(|| "https://s3.amazonaws.com".to_string())
}

fn is_aliyun_oss(endpoint_url: &str) -> bool {
    endpoint_url.contains("aliyuncs.com")
}

fn resolve_force_path_style(
    force_path_style: bool,
    endpoint_present: bool,
    endpoint_url: &str,
) -> bool {
    if force_path_style {
        true
    } else {
        endpoint_present && !is_aliyun_oss(endpoint_url)
    }
}

fn resolve_base_url(bucket: &str, endpoint_url: &str, force_path_style: bool) -> String {
    if is_aliyun_oss(endpoint_url) && !force_path_style {
        let host = endpoint_url
            .trim_start_matches("https://")
            .trim_start_matches("http://");
        format!("https://{}.{}", bucket, host)
    } else {
        endpoint_url.to_string()
    }
}

fn build_object_url(base_url: &str, bucket: &str, key: &str) -> String {
    format!("{}/{}/{}", base_url.trim_end_matches('/'), bucket, key)
}

impl S3Service {
    pub async fn new(
        bucket: String,
        region: String,
        endpoint: Option<String>,
        access_key_id: Option<String>,
        secret_access_key: Option<String>,
        force_path_style: bool,
    ) -> Self {
        let region_provider =
            RegionProviderChain::first_try(aws_types::region::Region::new(region))
                .or_default_provider();

        let mut config_loader =
            aws_config::defaults(BehaviorVersion::latest()).region(region_provider);

        if let (Some(ak), Some(sk)) = (access_key_id, secret_access_key) {
            let creds = Credentials::new(ak, sk, None, None, "config");
            config_loader =
                config_loader.credentials_provider(SharedCredentialsProvider::new(creds));
        }

        if let Some(ep) = endpoint.as_deref() {
            config_loader = config_loader.endpoint_url(normalize_endpoint(ep));
        }

        let config = config_loader.load().await;
        let endpoint_present = endpoint.is_some();
        let endpoint_for_url = endpoint_for_url(endpoint.as_deref());
        let final_force_path_style =
            resolve_force_path_style(force_path_style, endpoint_present, &endpoint_for_url);

        let s3_config = aws_sdk_s3::config::Builder::from(&config)
            .force_path_style(final_force_path_style)
            .build();

        let client = Client::from_conf(s3_config);

        let base_url = resolve_base_url(&bucket, &endpoint_for_url, final_force_path_style);
        Self {
            client,
            bucket,
            base_url,
        }
    }

    pub async fn upload_file(&self, key: &str, body: Vec<u8>) -> Result<String> {
        let md5_digest = md5::compute(&body);
        let base64_md5 = general_purpose::STANDARD.encode(md5_digest.0);

        let mut attempts = 0;
        loop {
            attempts += 1;
            match self.upload_file_internal(key, &body, &base64_md5).await {
                Ok(_) => break,
                Err(e) => {
                    if attempts >= 3 {
                        return Err(e);
                    }
                    tracing::warn!(attempt = attempts, error = ?e, "Upload failed, retrying");
                    tokio::time::sleep(std::time::Duration::from_millis(500 * attempts as u64))
                        .await;
                }
            }
        }

        Ok(build_object_url(&self.base_url, &self.bucket, key))
    }

    pub async fn get_presigned_url(
        &self,
        key: &str,
        expires_in: std::time::Duration,
    ) -> Result<String> {
        let presigned_request = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .presigned(
                PresigningConfig::expires_in(expires_in)
                    .map_err(|e| anyhow::anyhow!("Presign config error: {}", e))?,
            )
            .await
            .map_err(|e| anyhow::anyhow!("Presign error: {}", e))?;

        Ok(presigned_request.uri().to_string())
    }

    pub async fn download_file(&self, key: &str) -> Result<Vec<u8>> {
        let obj = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await?;
        let bytes = obj.body.collect().await?.into_bytes().to_vec();
        Ok(bytes)
    }

    async fn upload_file_internal(&self, key: &str, body: &[u8], base64_md5: &str) -> Result<()> {
        const MULTIPART_THRESHOLD: usize = 5 * 1024 * 1024;

        if body.len() > MULTIPART_THRESHOLD {
            self.upload_multipart(key, body).await
        } else {
            self.client
                .put_object()
                .bucket(&self.bucket)
                .key(key)
                .body(ByteStream::from(body.to_vec()))
                .content_md5(base64_md5)
                .send()
                .await?;
            Ok(())
        }
    }

    async fn upload_multipart(&self, key: &str, body: &[u8]) -> Result<()> {
        let create_multipart_upload_output = self
            .client
            .create_multipart_upload()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await?;

        let upload_id = create_multipart_upload_output
            .upload_id
            .ok_or_else(|| anyhow::anyhow!("No upload ID"))?;
        let mut completed_parts = Vec::new();
        let chunk_size = 5 * 1024 * 1024;

        for (i, chunk) in body.chunks(chunk_size).enumerate() {
            let part_number = (i + 1) as i32;
            let upload_part_output = self
                .client
                .upload_part()
                .bucket(&self.bucket)
                .key(key)
                .upload_id(&upload_id)
                .part_number(part_number)
                .body(ByteStream::from(chunk.to_vec()))
                .send()
                .await?;

            completed_parts.push(
                CompletedPart::builder()
                    .e_tag(upload_part_output.e_tag.unwrap_or_default())
                    .part_number(part_number)
                    .build(),
            );
        }

        self.client
            .complete_multipart_upload()
            .bucket(&self.bucket)
            .key(key)
            .upload_id(&upload_id)
            .multipart_upload(
                CompletedMultipartUpload::builder()
                    .set_parts(Some(completed_parts))
                    .build(),
            )
            .send()
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_helpers_normalize_and_default_values() {
        assert_eq!(
            normalize_endpoint("s3.example.com"),
            "https://s3.example.com"
        );
        assert_eq!(
            normalize_endpoint("\"http://localhost:9000\""),
            "http://localhost:9000"
        );
        assert_eq!(
            endpoint_for_url(None),
            "https://s3.amazonaws.com".to_string()
        );
    }

    #[test]
    fn force_path_style_and_base_url_follow_aliyun_rules() {
        let aliyun = "https://oss-cn-shanghai.aliyuncs.com";
        let custom = "https://minio.local";

        assert!(!resolve_force_path_style(false, true, aliyun));
        assert!(resolve_force_path_style(false, true, custom));
        assert!(resolve_force_path_style(true, false, aliyun));

        assert_eq!(
            resolve_base_url("bucket", aliyun, false),
            "https://bucket.oss-cn-shanghai.aliyuncs.com"
        );
        assert_eq!(
            resolve_base_url("bucket", custom, true),
            "https://minio.local".to_string()
        );
    }

    #[test]
    fn build_object_url_handles_trailing_slashes() {
        assert_eq!(
            build_object_url("https://s3.local/", "skills", "a.zip"),
            "https://s3.local/skills/a.zip"
        );
    }
}
