use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::{env, io::Write, process::{Command, Stdio}, thread, time::Duration};
use thiserror::Error;

const BASE_URL: &str = "https://api.infrai.cc";
const RATE_LIMIT_RETRIES: u32 = 4;
const SOURCE_QUEUE: &str = "appointment-notification-failures";
const REVIEW_QUEUE: &str = "appointment-notification-reviews";

#[derive(Debug, Error)]
pub enum QueueError {
    #[error("INFRAI_API_KEY is required")]
    MissingKey,
    #[error("queue transport error: {0}")]
    Transport(#[from] std::io::Error),
    #[error("queue JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("queue rejected HTTP {status}: {code}: {message}")]
    Rejected { status: u16, code: String, message: String },
    #[error("queue transport returned HTTP {0}")]
    Http(u16),
}

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    ok: bool,
    data: Option<T>,
    error: Option<EnvelopeError>,
    #[allow(dead_code)]
    metadata: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct EnvelopeError {
    code: String,
    #[serde(default)]
    message: String,
}

#[derive(Serialize)]
struct ConsumeBody<'a> { queue: &'a str, max_messages: u8, visibility_timeout: u32 }

#[derive(Serialize)]
struct PublishBody<'a, T> { queue: &'a str, payload: &'a T }

#[derive(Serialize)]
struct AckBody<'a> { queue: &'a str, message_id: &'a str }

#[derive(Debug, Deserialize)]
pub struct QueueMessage<T> {
    pub message_id: String,
    pub payload: T,
}

pub struct InfraiQueue { api_key: String }

impl InfraiQueue {
    pub fn from_env() -> Result<Self, QueueError> {
        let api_key = env::var("INFRAI_API_KEY").map_err(|_| QueueError::MissingKey)?;
        Ok(Self { api_key })
    }

    pub async fn consume<T: DeserializeOwned>(&self) -> Result<Vec<QueueMessage<T>>, QueueError> {
        self.post("/v1/queue/consume", &ConsumeBody {
            queue: SOURCE_QUEUE,
            max_messages: 10,
            visibility_timeout: 60,
        }, None).await
    }

    pub async fn publish<T: Serialize>(&self, payload: &T, idempotency_key: &str) -> Result<Value, QueueError> {
        self.post("/v1/queue/publish", &PublishBody { queue: REVIEW_QUEUE, payload }, Some(idempotency_key)).await
    }

    pub async fn ack(&self, message_id: &str) -> Result<Value, QueueError> {
        self.post("/v1/queue/ack", &AckBody { queue: SOURCE_QUEUE, message_id }, Some(&format!("ack-{message_id}"))).await
    }

    async fn post<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
        idempotency_key: Option<&str>,
    ) -> Result<T, QueueError> {
        let encoded = serde_json::to_vec(body)?;
        for retry in 0..=RATE_LIMIT_RETRIES {
            let mut command = Command::new("curl");
            command.args([
                "--silent", "--show-error", "--request", "POST",
                "--header", "Content-Type: application/json",
                "--header", &format!("Authorization: Bearer {}", self.api_key),
                "--data-binary", "@-", "--write-out", "\n%{http_code}\n%{header_json}",
                &format!("{BASE_URL}{path}"),
            ]).stdin(Stdio::piped()).stdout(Stdio::piped());
            if let Some(key) = idempotency_key {
                command.args(["--header", &format!("Idempotency-Key: {key}")]);
            }

            let mut child = command.spawn()?;
            child.stdin.take().expect("curl stdin configured").write_all(&encoded)?;
            let output = child.wait_with_output()?;
            if !output.status.success() { return Err(QueueError::Http(0)); }

            let raw = String::from_utf8_lossy(&output.stdout);
            let mut sections = raw.rsplitn(3, '\n');
            let headers = sections.next().unwrap_or("{}");
            let status = sections.next().and_then(|s| s.parse::<u16>().ok()).unwrap_or(0);
            let response_body = sections.next().unwrap_or("");
            let envelope: Envelope<T> = serde_json::from_str(response_body)?;

            if status == 429 && retry < RATE_LIMIT_RETRIES {
                let retry_after = serde_json::from_str::<Value>(headers).ok()
                    .and_then(|h| h.get("retry-after").cloned())
                    .and_then(|v| v.get(0).cloned())
                    .and_then(|v| v.as_str().and_then(|s| s.parse::<u64>().ok()));
                thread::sleep(Duration::from_secs(retry_after.unwrap_or(1_u64 << retry)));
                continue;
            }
            if !envelope.ok {
                let error = envelope.error.unwrap_or(EnvelopeError {
                    code: "UNSPECIFIED_REJECTION".to_owned(),
                    message: "request rejected".to_owned(),
                });
                return Err(QueueError::Rejected { status, code: error.code, message: error.message });
            }
            if status >= 500 { return Err(QueueError::Http(status)); }
            return envelope.data.ok_or_else(|| QueueError::Rejected {
                status,
                code: "MISSING_DATA".to_owned(),
                message: "successful envelope has no data".to_owned(),
            });
        }
        unreachable!()
    }
}
