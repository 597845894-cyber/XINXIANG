#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    DatabaseOpenFailed,
    DatabaseMigrationFailed,
    AttachmentContentUnavailable,
    ModelResourceUnavailable,
    AnalysisFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentVersion {
    StorageV1,
    SecurityV1,
    ModelResourcesV1,
    AnalysisV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurationBucket {
    UnderOneSecond,
    UnderFiveSeconds,
    UnderThirtySeconds,
    OverThirtySeconds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SafeLogEvent {
    pub error_code: ErrorCode,
    pub component_version: ComponentVersion,
    pub duration_bucket: DurationBucket,
}

pub trait SafeLogSink {
    fn write(&mut self, event: SafeLogEvent);
}

pub struct SafeLogger<S> {
    sink: S,
}

impl<S: SafeLogSink> SafeLogger<S> {
    pub fn new(sink: S) -> Self {
        Self { sink }
    }

    pub fn record(&mut self, event: SafeLogEvent) {
        self.sink.write(event);
    }

    pub fn into_sink(self) -> S {
        self.sink
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::{
        ComponentVersion, DurationBucket, ErrorCode, SafeLogEvent, SafeLogSink, SafeLogger,
    };

    #[derive(Default)]
    struct RecordingSink(Vec<SafeLogEvent>);

    impl SafeLogSink for RecordingSink {
        fn write(&mut self, event: SafeLogEvent) {
            self.0.push(event);
        }
    }

    #[test]
    fn log_records_are_structured_and_cannot_carry_sensitive_content() {
        let mut logger = SafeLogger::new(RecordingSink::default());
        logger.record(SafeLogEvent {
            error_code: ErrorCode::AnalysisFailed,
            component_version: ComponentVersion::AnalysisV1,
            duration_bucket: DurationBucket::UnderFiveSeconds,
        });
        let sink = logger.into_sink();

        assert_eq!(sink.0.len(), 1);
        assert_eq!(sink.0[0].error_code, ErrorCode::AnalysisFailed);
    }

    #[test]
    fn production_sources_do_not_bypass_the_safe_log_interface() {
        let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        assert_no_direct_log_calls(&source_root);
    }

    fn assert_no_direct_log_calls(directory: &Path) {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                assert_no_direct_log_calls(&path);
                continue;
            }
            if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
                continue;
            }
            let source = fs::read_to_string(&path).unwrap();
            assert!(!source.contains(&format!("print{}", "ln!")));
            assert!(!source.contains(&format!("eprint{}", "ln!")));
            assert!(!source.contains(&format!("tracing{}", "::")));
            assert!(!source.contains(&format!("log{}", "::")));
        }
    }
}
