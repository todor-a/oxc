// Instruction:
// run `cargo run -p oxc_resolver --example resolver -- `pwd` test.js`
// or `cargo watch -x "run -p oxc_resolver --example resolver" -- `pwd` test.js`
//
// For OXC_LOG, see syntax at https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#example-syntax
// e.g.
// * OXC_LOG=DEBUG command | grep ERROR
// * OXC_LOG=oxc_resolver[resolve{specifier=recursive-module}]=debug for a specifier specifier

use std::{env, path::PathBuf};

use oxc_resolver::{AliasValue, ResolveOptions, Resolver};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

fn main() {
    tracing_subscriber::registry().with(fmt::layer()).with(EnvFilter::from_env("OXC_LOG")).init();

    let path = env::args().nth(1).expect("require path");
    let request = env::args().nth(2).expect("require request");
    let path = PathBuf::from(path).canonicalize().unwrap();

    println!("path: {path:?}");
    println!("request: {request}");

    let options = ResolveOptions {
        alias: vec![("/asdf".into(), vec![AliasValue::Path("./test.js".into())])],
        ..ResolveOptions::default()
    };
    let resolved_path = Resolver::new(options).resolve(path, &request);

    println!("Result: {resolved_path:?}");
}
