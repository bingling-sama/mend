use crate::ring_buffer::RingBuffer;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PtyError {
    #[error("Failed to open PTY: {0}")]
    OpenFailed(String),
    #[error("Failed to spawn process in PTY: {0}")]
    SpawnFailed(String),
    #[error("Process wait failed: {0}")]
    WaitFailed(String),
}

/// Output captured from running a process inside a PTY
#[derive(Debug, Clone)]
pub struct PtyOutput {
    pub exit_code: i32,
    pub raw_output: Vec<u8>,
}

/// PtyRunner encapsulates running child commands inside a clean pseudo-terminal
/// without secondary re-runs or side-effects.
pub struct PtyRunner {
    ring_buffer_capacity: usize,
}

impl Default for PtyRunner {
    fn default() -> Self {
        Self::new(64 * 1024)
    }
}

impl PtyRunner {
    pub fn new(capacity: usize) -> Self {
        Self {
            ring_buffer_capacity: capacity,
        }
    }

    pub fn run(
        &self,
        command: &str,
        args: &[String],
        env_vars: &[(String, String)],
        cwd: Option<&str>,
        mirror_to_stdout: bool,
    ) -> Result<PtyOutput, PtyError> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| PtyError::OpenFailed(e.to_string()))?;

        let mut cmd = CommandBuilder::new(command);
        cmd.args(args);
        if let Some(dir) = cwd {
            cmd.cwd(dir);
        }
        for (k, v) in env_vars {
            cmd.env(k, v);
        }

        let mut child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| PtyError::SpawnFailed(e.to_string()))?;

        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| PtyError::OpenFailed(e.to_string()))?;

        let ring_buffer = Arc::new(Mutex::new(RingBuffer::new(self.ring_buffer_capacity)));
        let rb_clone = Arc::clone(&ring_buffer);

        let reader_thread = thread::spawn(move || {
            let mut buf = [0u8; 1024];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let bytes = &buf[..n];
                if mirror_to_stdout {
                    let _ = std::io::stdout().write_all(bytes);
                    let _ = std::io::stdout().flush();
                }
                if let Ok(mut rb) = rb_clone.lock() {
                    rb.write_all(bytes);
                }
            }
        });

        let status = child
            .wait()
            .map_err(|e| PtyError::WaitFailed(e.to_string()))?;

        let _ = reader_thread.join();

        let exit_code = if status.success() {
            0
        } else {
            status.exit_code() as i32
        };

        let raw_output = ring_buffer.lock().unwrap().to_vec();

        Ok(PtyOutput {
            exit_code,
            raw_output,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pty_runner_echo() {
        let runner = PtyRunner::new(1024);
        let output = runner
            .run("echo", &["hello_pty".to_string()], &[], None, false)
            .expect("Pty execution should succeed");
        assert_eq!(output.exit_code, 0);
        let text = String::from_utf8_lossy(&output.raw_output);
        assert!(text.contains("hello_pty"));
    }

    #[test]
    fn test_pty_runner_failure() {
        let runner = PtyRunner::new(1024);
        let output = runner
            .run("sh", &["-c".to_string(), "exit 42".to_string()], &[], None, false)
            .expect("Pty execution should succeed");
        assert_eq!(output.exit_code, 42);
    }
}
