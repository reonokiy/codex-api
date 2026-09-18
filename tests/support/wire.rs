//! Captures actual TCP byte streams between the test gateway and mock upstream.
//! This is not an IP packet capture: segmentation and timing are intentionally not asserted.
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

type Bytes = Arc<Mutex<Vec<u8>>>;
#[derive(Clone, Default)]
struct Connection {
    request: Bytes,
    response: Bytes,
}
pub struct Capture {
    pub url: String,
    connections: Arc<Mutex<Vec<Connection>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Capture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Capture {
    pub async fn start(destination: String) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let connections = Arc::new(Mutex::new(Vec::new()));
        let captured = connections.clone();
        let task = tokio::spawn(async move {
            let mut tasks = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    accepted=listener.accept() => {
                        let Ok((client,_))=accepted else { break; };
                        let destination=destination.clone();
                        let connection=Connection::default();
                        captured.lock().unwrap().push(connection.clone());
                        tasks.spawn(async move {
                            let server=tokio::net::TcpStream::connect(destination.trim_start_matches("http://")).await.unwrap();
                            let (mut cr,mut cw)=client.into_split();
                            let (mut sr,mut sw)=server.into_split();
                            let upstream=copy(&mut cr,&mut sw,connection.request);
                            let downstream=copy(&mut sr,&mut cw,connection.response);
                            let _=tokio::join!(upstream,downstream);
                        });
                    }
                    _=tasks.join_next(), if !tasks.is_empty() => {},
                }
            }
        });
        Self {
            url,
            connections,
            task,
        }
    }
    pub fn save(&self, name: &str) -> std::path::PathBuf {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("artifacts/protocol")
            .join(name);
        std::fs::create_dir_all(&directory).unwrap();
        let connections = self.connections.lock().unwrap();
        for (index, connection) in connections.iter().enumerate() {
            for (direction, bytes) in [
                ("request", &connection.request),
                ("response", &connection.response),
            ] {
                let bytes = redact(&bytes.lock().unwrap());
                std::fs::write(directory.join(format!("{index}-{direction}.tcp")), bytes).unwrap();
            }
        }
        directory
    }
}
async fn copy<R: tokio::io::AsyncRead + Unpin, W: tokio::io::AsyncWrite + Unpin>(
    read: &mut R,
    write: &mut W,
    capture: Bytes,
) -> std::io::Result<()> {
    let mut buffer = [0u8; 8192];
    loop {
        let count = read.read(&mut buffer).await?;
        if count == 0 {
            write.shutdown().await?;
            return Ok(());
        }
        capture.lock().unwrap().extend_from_slice(&buffer[..count]);
        write.write_all(&buffer[..count]).await?;
    }
}
fn redact(bytes: &[u8]) -> Vec<u8> {
    let mut output = bytes.to_vec();
    // Captures use synthetic credentials only. Also mask any credential headers in artifacts.
    for name in [
        b"authorization:".as_slice(),
        b"chatgpt-account-id:".as_slice(),
        b"x-oai-attestation:".as_slice(),
    ] {
        let lower = bytes.to_ascii_lowercase();
        for start in 0..lower.len().saturating_sub(name.len()) {
            if lower[start..].starts_with(name) && (start == 0 || lower[start - 1] == b'\n') {
                let from = start + name.len();
                let end = bytes[from..]
                    .iter()
                    .position(|v| *v == b'\r' || *v == b'\n')
                    .map(|n| from + n)
                    .unwrap_or(bytes.len());
                output[from..end].fill(b'*');
            }
        }
    }
    output
}
