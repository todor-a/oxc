use crate::tester::Tester;
use crate::{rule::RuleMeta, rules::eslint::no_unused_vars::NoUnusedVars};
use serde_json::json;

#[test]
fn test_function_simple() {
    let pass = vec![
        "function foo() { return }; foo()",
        "export function foo() { return }",
        "export default function foo() { return }",
    ];
    let fail = vec!["function foo() { return }", "function foo() { return foo() }"];
    Tester::new_without_config(NoUnusedVars::NAME, pass, fail).test();
}

#[test]
fn test_class_simple() {
    let pass = vec![
        // ("class Foo {}; const f = new Foo(); console.log(f)"),
        ("export class Foo {}"),
        // ("export default class Foo {}"),
    ];
    let fail = vec![("class Foo {}")];
    Tester::new_without_config(NoUnusedVars::NAME, pass, fail).test();
}

#[test]
fn test_class_advanced() {
    let pass = vec![
        "
        export class Foo {
            constructor(x) {
                this._x = x;
            }

            getX() {
                return this.x
            }
        }
        ",
    ];
    let fail = vec![
        // todo
        // "
        // class Foo {
        //     static fooFactory() {
        //         return new Foo();
        //     }
        // }
        // ",
    ];

    Tester::new_without_config(NoUnusedVars::NAME, pass, fail).test();
}

#[test]
fn test_args_simple() {
    let all = Some(json!([{ "args": "all" }]));
    let none = Some(json!([{ "args": "none" }]));
    let after_used = Some(json!([{ "args": "after-used" }]));
    let pass = vec![
        // ("function foo(a) { return a }; foo()", all.clone()),
        // ("function foo(a) { return }; foo()", none),
        // after used
        ("function foo(a, b) { return b }; foo()", None),
        ("function foo(a, b) { return b }; foo()", after_used),
    ];
    let fail = vec![
        // ("function foo(a) { return }; foo()", None),
        // ("function foo(a = 1) { return }; foo()", None),
        // ("function foo() { return }", None),
        // ("function foo(a, b) { return a }; foo()", None),
        // ("function foo(a, b) { return b }; foo()", all),
    ];
    Tester::new(NoUnusedVars::NAME, pass, fail).test();
}

#[test]
fn test_args_unpacking() {
    let ignore_rest_siblings = Some(json!([{ "ignoreRestSiblings": true }]));
    let ignore_arg_underscore = Some(json!([{ "destructuredArrayIgnorePattern": "^_" }]));
    let pass = vec![
        ("function baz([_b, foo]) { foo; };\nbaz()", ignore_arg_underscore),
        (
            "let { foo, ...rest } = something;
                console.log(rest);",
            ignore_rest_siblings.clone(),
        ),
        ("let foo, rest; ({ foo, ...rest } = something); console.log(rest);", ignore_rest_siblings),
    ];
    let fail = vec![];
    Tester::new(NoUnusedVars::NAME, pass, fail);
}

#[test]
fn test_args_in_setters() {
    let pass = vec![
        ("f({ set foo(a) { return; } });", None),
        ("(class { set foo(UNUSED) {} })", None),
        ("class Foo { set bar(UNUSED) {} } console.log(Foo)", None),
    ];
    let fail = vec![];

    Tester::new(NoUnusedVars::NAME, pass, fail).test();
}
