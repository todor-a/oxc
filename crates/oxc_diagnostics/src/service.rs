use std::{
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc,
    },
};

use crate::{miette::NamedSource, Error, GraphicalReportHandler, MinifiedFileError, Severity};

pub type DiagnosticTuple = (PathBuf, Vec<Error>);
pub type DiagnosticSender = mpsc::Sender<DiagnosticTuple>;
pub type DiagnosticReceiver = mpsc::Receiver<DiagnosticTuple>;

pub struct DiagnosticService {
    /// Disable reporting on warnings, only errors are reported
    quiet: bool,

    /// Specify a warning threshold,
    /// which can be used to force exit with an error status if there are too many warning-level rule violations in your project
    max_warnings: Option<usize>,

    /// Total number of warnings received
    warnings_count: AtomicUsize,

    /// Total number of errors received
    errors_count: AtomicUsize,
}

impl Default for DiagnosticService {
    fn default() -> Self {
        Self {
            quiet: false,
            max_warnings: None,
            warnings_count: AtomicUsize::new(0),
            errors_count: AtomicUsize::new(0),
        }
    }
}

impl DiagnosticService {
    #[must_use]
    pub fn with_quiet(mut self, yes: bool) -> Self {
        self.quiet = yes;
        self
    }

    #[must_use]
    pub fn with_max_warnings(mut self, max_warnings: Option<usize>) -> Self {
        self.max_warnings = max_warnings;
        self
    }

    pub fn channel() -> (DiagnosticSender, DiagnosticReceiver) {
        mpsc::channel()
    }

    pub fn warnings_count(&self) -> usize {
        self.warnings_count.load(Ordering::SeqCst)
    }

    pub fn errors_count(&self) -> usize {
        self.errors_count.load(Ordering::SeqCst)
    }

    pub fn max_warnings_exceeded(&self) -> bool {
        self.max_warnings.map_or(false, |max_warnings| self.warnings_count() > max_warnings)
    }

    pub fn wrap_diagnostics(
        path: &Path,
        source_text: &str,
        diagnostics: Vec<Error>,
    ) -> (PathBuf, Vec<Error>) {
        let source = Arc::new(NamedSource::new(path.to_string_lossy(), source_text.to_owned()));
        let diagnostics = diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.with_source_code(Arc::clone(&source)))
            .collect();
        (path.to_path_buf(), diagnostics)
    }

    /// # Panics
    ///
    /// * When the writer fails to write
    pub fn run(&self, rx_error: &DiagnosticReceiver) {
        let mut buf_writer = BufWriter::new(std::io::stdout());
        let handler = GraphicalReportHandler::new();

        while let Ok((path, diagnostics)) = rx_error.recv() {
            let mut output = String::new();
            for diagnostic in diagnostics {
                let severity = diagnostic.severity();
                let is_warning = severity == Some(Severity::Warning);
                let is_error = severity.is_none() || severity == Some(Severity::Error);
                if is_warning || is_error {
                    if is_warning {
                        self.warnings_count.fetch_add(1, Ordering::SeqCst);
                    }
                    if is_error {
                        self.errors_count.fetch_add(1, Ordering::SeqCst);
                    }
                    // The --quiet flag follows ESLint's --quiet behavior as documented here: https://eslint.org/docs/latest/use/command-line-interface#--quiet
                    // Note that it does not disable ALL diagnostics, only Warning diagnostics
                    if self.quiet {
                        continue;
                    }

                    if let Some(max_warnings) = self.max_warnings {
                        if self.warnings_count() > max_warnings {
                            continue;
                        }
                    }
                }

                let mut err = String::new();
                handler.render_report(&mut err, diagnostic.as_ref()).unwrap();
                // Skip large output and print only once
                if err.lines().any(|line| line.len() >= 400) {
                    let minified_diagnostic = Error::new(MinifiedFileError(path.clone()));
                    err = format!("{minified_diagnostic:?}");
                    output = err;
                    break;
                }
                output.push_str(&err);
            }
            buf_writer.write_all(output.as_bytes()).unwrap();
        }

        buf_writer.flush().unwrap();
    }
}
