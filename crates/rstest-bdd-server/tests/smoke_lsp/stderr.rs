//! Bounded stderr capture for smoke-test language-server processes.

use std::{
    io::Read,
    sync::{
        Arc,
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
};

/// Maximum number of server stderr bytes retained for a failed smoke test.
const MAX_CAPTURED_STDERR_BYTES: usize = 16 * 1024;

/// A shareable view of a running server's captured standard error.
#[derive(Clone)]
pub struct StderrDiagnostics {
    bytes: Arc<Mutex<Vec<u8>>>,
    is_truncated: Arc<AtomicBool>,
}

impl StderrDiagnostics {
    /// Render the bounded stderr diagnostic for a test failure.
    pub fn render(&self) -> String {
        let Ok(bytes) = self.bytes.lock() else {
            return "LSP stderr: <unavailable>".to_owned();
        };
        let detail = String::from_utf8_lossy(&bytes).trim().to_owned();
        if detail.is_empty() {
            return "LSP stderr: <empty>".to_owned();
        }
        if self.is_truncated.load(Ordering::Relaxed) {
            return format!(
                "LSP stderr:\n{detail}\n[truncated after {MAX_CAPTURED_STDERR_BYTES} bytes]"
            );
        }
        format!("LSP stderr:\n{detail}")
    }

    fn record(&self, bytes: &[u8]) {
        let Ok(mut captured) = self.bytes.lock() else {
            return;
        };
        let remaining = MAX_CAPTURED_STDERR_BYTES.saturating_sub(captured.len());
        let captured_len = bytes.len().min(remaining);
        if let Some(prefix) = bytes.get(..captured_len) {
            captured.extend_from_slice(prefix);
        }
        if captured_len < bytes.len() {
            self.is_truncated.store(true, Ordering::Relaxed);
        }
    }
}

/// Owns the background reader that preserves a server's standard error.
pub struct StderrCapture {
    diagnostics: StderrDiagnostics,
    reader: Option<JoinHandle<()>>,
}

impl StderrCapture {
    /// Start draining stderr without mixing it into JSON-RPC stdout.
    pub fn spawn(mut reader: impl Read + Send + 'static) -> Self {
        let diagnostics = StderrDiagnostics {
            bytes: Arc::new(Mutex::new(Vec::new())),
            is_truncated: Arc::new(AtomicBool::new(false)),
        };
        let capture = diagnostics.clone();
        let reader = std::thread::spawn(move || {
            let mut buffer = [0_u8; 1024];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => {
                        if let Some(chunk) = buffer.get(..read) {
                            capture.record(chunk);
                        }
                    }
                }
            }
        });
        Self {
            diagnostics,
            reader: Some(reader),
        }
    }

    /// Return a shareable view for timeout diagnostics while the child runs.
    pub fn diagnostics(&self) -> StderrDiagnostics { self.diagnostics.clone() }

    /// Wait for the stderr reader after the child exits, then render its output.
    pub fn finish(&mut self) -> String {
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        self.diagnostics.render()
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for early language-server stderr diagnostics.

    use std::{
        io::Write,
        process::{Command, Stdio},
    };

    use super::StderrCapture;

    const STDERR_CHILD_ENV: &str = "RSTEST_BDD_SMOKE_LSP_STDERR_CHILD";
    const STDERR_MARKER: &str = "known LSP stderr marker";

    #[test]
    fn captures_stderr_from_an_early_exiting_child() {
        if std::env::var_os(STDERR_CHILD_ENV).is_some() {
            let _ = writeln!(std::io::stderr().lock(), "{STDERR_MARKER}");
            std::process::exit(1);
        }

        let Ok(current_exe) = std::env::current_exe() else {
            panic!("test binary path should be available");
        };
        let Ok(mut child) = Command::new(current_exe)
            .args([
                "stderr::tests::captures_stderr_from_an_early_exiting_child",
                "--exact",
                "--nocapture",
            ])
            .env(STDERR_CHILD_ENV, "1")
            .stderr(Stdio::piped())
            .spawn()
        else {
            panic!("controlled stderr child should start");
        };
        let Some(stderr) = child.stderr.take() else {
            panic!("controlled stderr child should have a piped stderr");
        };
        let mut capture = StderrCapture::spawn(stderr);
        let Ok(status) = child.wait() else {
            panic!("controlled stderr child should exit");
        };

        assert!(!status.success(), "controlled child should fail");
        assert!(capture.finish().contains(STDERR_MARKER));
    }
}
