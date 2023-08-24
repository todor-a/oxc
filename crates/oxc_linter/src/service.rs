use std::{
    collections::HashMap,
    fs,
    path::Path,
    rc::Rc,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Condvar, Mutex,
    },
};

use oxc_allocator::Allocator;
use oxc_diagnostics::{DiagnosticSender, DiagnosticService};
use oxc_parser::Parser;
use oxc_resolver::{ResolveOptions, Resolver};
use oxc_semantic::{ModuleRecordBuilder, SemanticBuilder};
use oxc_span::{SourceType, VALID_EXTENSIONS};
use oxc_syntax::module_record::ModuleRecord;

use dashmap::DashMap;
use rayon::{iter::ParallelIterator, prelude::ParallelBridge};

use crate::{Fixer, LintContext, Linter, Message};

type ModuleMap = DashMap<Box<Path>, Arc<ModuleRecord>>;

pub struct LintService {
    runtime: Runtime,
}

impl LintService {
    pub fn number_of_files_in_node_modules(&self) -> usize {
        self.runtime.number_of_files_in_node_modules.load(Ordering::SeqCst)
    }

    pub fn new(linter: Arc<Linter>) -> Self {
        let runtime = Runtime {
            linter,
            resolver: Arc::new(Self::resolver()),
            module_map: Arc::default(),
            cache_state: Arc::default(),
            number_of_files_in_node_modules: Arc::default(),
        };
        Self { runtime }
    }

    fn resolver() -> Resolver {
        Resolver::new(ResolveOptions {
            condition_names: vec!["node".into(), "import".into()],
            extension_alias: vec![
                (".js".into(), vec![".js".into(), ".tsx".into(), "ts".into()]),
                (".mjs".into(), vec![".mjs".into(), ".mts".into()]),
            ],
            extensions: VALID_EXTENSIONS.iter().map(|ext| format!(".{ext}")).collect(),
            ..ResolveOptions::default()
        })
    }

    pub fn run_path(&self, path: &Path, tx_error: &DiagnosticSender) {
        self.runtime.process_path(path, tx_error);
    }

    pub fn run_source<'a>(
        &self,
        path: &Path,
        allocator: &'a Allocator,
        source_text: &'a str,
        check_syntax_errors: bool,
        tx_error: &DiagnosticSender,
    ) -> Vec<Message<'a>> {
        let Ok(source_type) = SourceType::from_path(path) else { return vec![] };
        self.runtime.init_cache_state(path);
        self.runtime.process_source(
            path,
            allocator,
            source_text,
            source_type,
            check_syntax_errors,
            tx_error,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CacheStateEntry {
    ReadyToConstruct,
    PendingStore(usize),
}

type CacheState = Mutex<HashMap<Box<Path>, Arc<(Mutex<CacheStateEntry>, Condvar)>>>;

#[derive(Clone)]
pub struct Runtime {
    linter: Arc<Linter>,
    resolver: Arc<Resolver>,
    module_map: Arc<ModuleMap>,
    cache_state: Arc<CacheState>,
    number_of_files_in_node_modules: Arc<AtomicUsize>,
}

impl Runtime {
    fn process_path(&self, path: &Path, tx_error: &DiagnosticSender) {
        let Ok(source_type) = SourceType::from_path(path) else { return };

        if self.module_map.contains_key(path) {
            return;
        }

        self.init_cache_state(path);

        let allocator = Allocator::default();
        let source_text =
            fs::read_to_string(path).unwrap_or_else(|_| panic!("Failed to read {path:?}"));

        let mut messages =
            self.process_source(path, &allocator, &source_text, source_type, true, tx_error);

        if self.linter.options().fix {
            let fix_result = Fixer::new(&source_text, messages).fix();
            fs::write(path, fix_result.fixed_code.as_bytes()).unwrap();
            messages = fix_result.messages;
        }

        if !messages.is_empty() {
            let errors = messages.into_iter().map(|m| m.error).collect();
            let diagnostics = DiagnosticService::wrap_diagnostics(path, &source_text, errors);
            tx_error.send(diagnostics).unwrap();
        }
    }

    fn process_source<'a>(
        &self,
        path: &Path,
        allocator: &'a Allocator,
        source_text: &'a str,
        source_type: SourceType,
        check_syntax_errors: bool,
        tx_error: &DiagnosticSender,
    ) -> Vec<Message<'a>> {
        let ret = Parser::new(allocator, source_text, source_type)
            .allow_return_outside_function(true)
            .parse();

        if !ret.errors.is_empty() {
            return ret.errors.into_iter().map(|err| Message::new(err, None)).collect();
        };

        let mut module_record_builder = ModuleRecordBuilder::default();
        module_record_builder.visit(&ret.program);
        let module_record = Arc::new(module_record_builder.build());
        self.module_map.insert(path.to_path_buf().into_boxed_path(), Arc::clone(&module_record));

        self.update_cache_state(path);

        let tx_error = tx_error.clone();
        let canonicalized = path.canonicalize().unwrap();
        let dir = canonicalized.parent().unwrap();
        let cwd = std::env::current_dir().unwrap();

        module_record
            .module_requests
            .keys()
            .cloned()
            .par_bridge()
            .map_with(&self.resolver, |resolver, specifier| resolver.resolve(&dir, &specifier).ok())
            .flatten()
            .filter(|r| !self.module_map.contains_key(r.path()))
            .for_each_with(tx_error, |tx_error, resolution| {
                self.process_path(resolution.path().strip_prefix(&cwd).unwrap(), tx_error);
            });

        if path.components().any(|p| p.as_os_str() == "node_modules") {
            self.number_of_files_in_node_modules.fetch_add(1, Ordering::SeqCst);
            return vec![];
        }

        let program = allocator.alloc(ret.program);

        let semantic_ret = SemanticBuilder::new(source_text, source_type)
            .with_trivias(ret.trivias)
            .with_check_syntax_error(check_syntax_errors)
            .with_module_record_builder(false)
            .build(program);

        if !semantic_ret.errors.is_empty() {
            return semantic_ret.errors.into_iter().map(|err| Message::new(err, None)).collect();
        };

        let lint_ctx = LintContext::new(&Rc::new(semantic_ret.semantic));
        self.linter.run(lint_ctx)
    }

    // https://medium.com/@polyglot_factotum/rust-concurrency-patterns-condvars-and-locks-e278f18db74f
    fn init_cache_state(&self, path: &Path) {
        let (lock, cvar) = {
            let mut state_map = self.cache_state.lock().unwrap();
            &*Arc::clone(state_map.entry(path.to_path_buf().into_boxed_path()).or_insert_with(
                || Arc::new((Mutex::new(CacheStateEntry::ReadyToConstruct), Condvar::new())),
            ))
        };

        let mut state = cvar
            .wait_while(lock.lock().unwrap(), |state| {
                matches!(*state, CacheStateEntry::PendingStore(_))
            })
            .unwrap();

        if self.module_map.get(path).is_none() {
            let i = if let CacheStateEntry::PendingStore(i) = *state { i } else { 0 };
            *state = CacheStateEntry::PendingStore(i + 1);
        }

        if *state == CacheStateEntry::ReadyToConstruct {
            cvar.notify_one();
        }
    }

    fn update_cache_state(&self, path: &Path) {
        let (lock, cvar) = {
            let mut state_map = self.cache_state.lock().unwrap();
            &*Arc::clone(
                state_map
                    .get_mut(path)
                    .expect("Entry in http-cache state to have been previously inserted"),
            )
        };
        let mut state = lock.lock().unwrap();
        if let CacheStateEntry::PendingStore(i) = *state {
            let new = i - 1;
            if new == 0 {
                *state = CacheStateEntry::ReadyToConstruct;
                // Notify the next thread waiting in line, if there is any.
                cvar.notify_one();
            } else {
                *state = CacheStateEntry::PendingStore(new);
            }
        }
    }
}
