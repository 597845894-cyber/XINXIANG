use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalInferenceError {
    EmptyInput,
    RuntimeMissing,
    ModelMissing,
    Busy,
    Timeout,
    Cancelled,
    Crashed,
    Io,
}

pub struct LocalInferenceAdapter {
    runtime: PathBuf,
    model: PathBuf,
    gate: Mutex<()>,
}

impl LocalInferenceAdapter {
    pub fn from_root(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref();
        Self {
            runtime: root.join("runtime/llama-cli.exe"),
            model: root.join("semantic/qwen2.5-1.5b-instruct-q4_k_m.gguf"),
            gate: Mutex::new(()),
        }
    }

    pub fn analyze(
        &self,
        prompt: &str,
        timeout: Duration,
        cancellation: Arc<AtomicBool>,
    ) -> Result<String, LocalInferenceError> {
        if prompt.trim().is_empty() {
            return Err(LocalInferenceError::EmptyInput);
        }
        if !self.runtime.is_file() {
            return Err(LocalInferenceError::RuntimeMissing);
        }
        if !self.model.is_file() {
            return Err(LocalInferenceError::ModelMissing);
        }
        let _guard = self
            .gate
            .try_lock()
            .map_err(|_| LocalInferenceError::Busy)?;
        let mut child = Command::new(&self.runtime)
            .args([
                "--model",
                self.model.to_string_lossy().as_ref(),
                "--simple-io",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| LocalInferenceError::Io)?;
        child
            .stdin
            .take()
            .ok_or(LocalInferenceError::Io)?
            .write_all(prompt.as_bytes())
            .map_err(|_| LocalInferenceError::Io)?;

        let started = Instant::now();
        loop {
            if cancellation.load(Ordering::Relaxed) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(LocalInferenceError::Cancelled);
            }
            if started.elapsed() >= timeout {
                let _ = child.kill();
                let _ = child.wait();
                return Err(LocalInferenceError::Timeout);
            }
            match child.try_wait().map_err(|_| LocalInferenceError::Io)? {
                Some(status) if status.success() => {
                    let output = child
                        .wait_with_output()
                        .map_err(|_| LocalInferenceError::Io)?;
                    return String::from_utf8(output.stdout)
                        .map_err(|_| LocalInferenceError::Crashed);
                }
                Some(_) => return Err(LocalInferenceError::Crashed),
                None => thread::sleep(Duration::from_millis(20)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LocalInferenceAdapter, LocalInferenceError};
    use std::{
        fs,
        sync::{atomic::AtomicBool, Arc},
        time::Duration,
    };
    use tempfile::tempdir;

    #[cfg(windows)]
    fn runtime_script(path: &std::path::Path, body: &str) {
        fs::write(path, format!("@echo off\r\necho {body}\r\n")).unwrap();
    }

    #[test]
    fn rejects_missing_local_resources_without_network_fallback() {
        let directory = tempdir().unwrap();
        let adapter = LocalInferenceAdapter::from_root(directory.path());
        assert_eq!(
            adapter.analyze(
                "通知",
                Duration::from_millis(20),
                Arc::new(AtomicBool::new(false))
            ),
            Err(LocalInferenceError::RuntimeMissing)
        );
    }

    #[test]
    fn rejects_empty_prompt_before_starting_a_process() {
        let directory = tempdir().unwrap();
        let adapter = LocalInferenceAdapter::from_root(directory.path());
        assert_eq!(
            adapter.analyze(
                "  ",
                Duration::from_millis(20),
                Arc::new(AtomicBool::new(false))
            ),
            Err(LocalInferenceError::EmptyInput)
        );
    }
}
