use std::{
    io::{Read, Write},
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

    #[cfg(test)]
    fn with_paths(runtime: PathBuf, model: PathBuf) -> Self {
        Self {
            runtime,
            model,
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
        let mut command = if self
            .runtime
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("cmd"))
        {
            let mut command = Command::new("cmd.exe");
            command.args(["/C", self.runtime.to_string_lossy().as_ref()]);
            command
        } else {
            Command::new(&self.runtime)
        };
        let mut child = command
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
                    let mut output = Vec::new();
                    child
                        .stdout
                        .take()
                        .ok_or(LocalInferenceError::Io)?
                        .read_to_end(&mut output)
                        .map_err(|_| LocalInferenceError::Io)?;
                    return String::from_utf8(output).map_err(|_| LocalInferenceError::Crashed);
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

    #[cfg(windows)]
    fn adapter_fixture(body: &str) -> (tempfile::TempDir, LocalInferenceAdapter) {
        let directory = tempdir().unwrap();
        let runtime = directory.path().join("fake-runtime.cmd");
        let model = directory.path().join("semantic/qwen.gguf");
        fs::create_dir_all(model.parent().unwrap()).unwrap();
        fs::write(&model, b"fixture").unwrap();
        fs::write(&runtime, format!("@echo off\r\n{body}\r\n")).unwrap();
        let adapter = LocalInferenceAdapter::with_paths(runtime, model);
        (directory, adapter)
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

    #[cfg(windows)]
    #[test]
    fn returns_single_structured_response_from_local_runtime() {
        let (_directory, adapter) = adapter_fixture(
            r#"echo {"category":"required-action","changeIntent":"none","tasks":[],"uncertainties":[]}"#,
        );
        let output = adapter
            .analyze(
                "通知原文",
                Duration::from_secs(1),
                Arc::new(AtomicBool::new(false)),
            )
            .unwrap();
        assert!(output.contains("required-action"));
    }

    #[cfg(windows)]
    #[test]
    fn terminates_runtime_on_timeout() {
        let (_directory, adapter) = adapter_fixture("ping -n 10 127.0.0.1 >nul");
        assert_eq!(
            adapter.analyze(
                "通知原文",
                Duration::from_millis(20),
                Arc::new(AtomicBool::new(false)),
            ),
            Err(LocalInferenceError::Timeout)
        );
    }

    #[cfg(windows)]
    #[test]
    fn terminates_runtime_when_cancelled() {
        let (_directory, adapter) = adapter_fixture("ping -n 10 127.0.0.1 >nul");
        let cancellation = Arc::new(AtomicBool::new(false));
        let signal = cancellation.clone();
        let worker = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            signal.store(true, std::sync::atomic::Ordering::Relaxed);
        });
        assert_eq!(
            adapter.analyze("通知原文", Duration::from_secs(2), cancellation),
            Err(LocalInferenceError::Cancelled)
        );
        worker.join().unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn reports_runtime_crash_without_partial_output() {
        let (_directory, adapter) = adapter_fixture("exit /b 1");
        assert_eq!(
            adapter.analyze(
                "通知原文",
                Duration::from_secs(1),
                Arc::new(AtomicBool::new(false)),
            ),
            Err(LocalInferenceError::Crashed)
        );
    }
}
