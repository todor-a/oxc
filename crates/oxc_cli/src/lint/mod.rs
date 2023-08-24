use std::{
    io::BufWriter,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use oxc_diagnostics::DiagnosticService;
use oxc_linter::{LintOptions, LintService, Linter};

use crate::{command::LintOptions as CliLintOptions, walk::Walk, CliRunResult, Runner};

pub struct LintRunner {
    options: CliLintOptions,
}

impl Runner for LintRunner {
    type Options = CliLintOptions;

    fn new(options: Self::Options) -> Self {
        Self { options }
    }

    fn run(self) -> CliRunResult {
        if self.options.misc_options.rules {
            let mut stdout = BufWriter::new(std::io::stdout());
            Linter::print_rules(&mut stdout);
            return CliRunResult::None;
        }

        let CliLintOptions {
            paths,
            filter,
            warning_options,
            ignore_options,
            fix_options,
            misc_options,
        } = self.options;

        let now = std::time::Instant::now();

        let lint_options = LintOptions::default()
            .with_filter(filter)
            .with_fix(fix_options.fix)
            .with_timing(misc_options.timing);

        let linter = Arc::new(Linter::from_options(lint_options));

        let diagnostic_service = DiagnosticService::default()
            .with_quiet(warning_options.quiet)
            .with_max_warnings(warning_options.max_warnings);
        let diagnostic_service = Arc::new(diagnostic_service);

        let number_of_files = Arc::new(AtomicUsize::new(0));

        let lint_service = Arc::new(LintService::new(Arc::clone(&linter)));

        rayon::scope({
            let diagnostic_service = Arc::clone(&diagnostic_service);
            let number_of_files = Arc::clone(&number_of_files);
            let lint_service = Arc::clone(&lint_service);
            let walk = Walk::new(&paths, &ignore_options);
            let (tx_error, rx_error) = DiagnosticService::channel();
            move |s| {
                s.spawn(move |_| {
                    diagnostic_service.run(&rx_error);
                });

                let mut count = 0;
                for path in walk.iter() {
                    count += 1;
                    let lint_service = Arc::clone(&lint_service);
                    let tx_error = tx_error.clone();
                    s.spawn(move |_| {
                        // let path = path.canonicalize().unwrap();
                        lint_service.run_path(&path, &tx_error);
                    });
                }
                number_of_files.store(count, Ordering::SeqCst);
            }
        });

        linter.print_execution_times_if_enable();

        CliRunResult::LintResult {
            duration: now.elapsed(),
            number_of_rules: linter.number_of_rules(),
            number_of_files: number_of_files.load(Ordering::SeqCst),
            number_of_warnings: diagnostic_service.warnings_count(),
            number_of_errors: diagnostic_service.errors_count(),
            max_warnings_exceeded: diagnostic_service.max_warnings_exceeded(),
        }
    }
}
