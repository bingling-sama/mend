use heal_core::{ExecutionState, RemediationCandidate};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

pub const DEFAULT_SOCKET_PATH: &str = "/tmp/jev-heal.sock";

#[derive(Debug, Serialize, Deserialize)]
pub struct DaemonRequest {
    pub state: ExecutionState,
    pub is_agent: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DaemonResponse {
    pub candidate: Option<RemediationCandidate>,
    pub error: Option<String>,
}

pub struct UdsClient {
    socket_path: PathBuf,
}

impl UdsClient {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: path.into(),
        }
    }

    pub fn default_client() -> Self {
        Self::new(DEFAULT_SOCKET_PATH)
    }

    pub async fn request(&self, req: &DaemonRequest) -> Result<DaemonResponse, Box<dyn std::error::Error + Send + Sync>> {
        let mut stream = UnixStream::connect(&self.socket_path).await?;
        let payload = serde_json::to_vec(req)?;
        let len = payload.len() as u32;

        stream.write_all(&len.to_be_bytes()).await?;
        stream.write_all(&payload).await?;

        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await?;
        let resp_len = u32::from_be_bytes(len_buf) as usize;

        let mut resp_buf = vec![0u8; resp_len];
        stream.read_exact(&mut resp_buf).await?;

        let resp: DaemonResponse = serde_json::from_slice(&resp_buf)?;
        Ok(resp)
    }
}
