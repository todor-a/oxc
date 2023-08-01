use crate::tester::Tester;
use crate::{rule::RuleMeta, rules::eslint::no_unused_vars::NoUnusedVars};
use serde_json::json;

#[test]
fn test_var_simple() {
    let pass = vec![
        "let a = 1; console.log(a);",
        "let a = 1; let b = a + 1; console.log(b);",
        "export var foo = 123;",
    ];
    let fail = vec![
        // cargo fmt pls
        "let a",
        "let a = 1;",
        "let a = 1; a += 2;",
        "let a = 1; a = a + 1;",
    ];

    Tester::new_without_config(NoUnusedVars::NAME, pass, fail).test();
}

#[test]
fn test_var_assignment() {
    let pass = vec![
        "let a = 1; b = a + 1; console.log(b)",
        "let a = 1; b = ++a; console.log(b)",
        // "export const Foo = class Bar {}",
    ];
    let fail = vec![];
    Tester::new_without_config(NoUnusedVars::NAME, pass, fail).test();
}

#[test]
fn test_var_simple_scoped() {
    let pass = vec!["let a = 1; function foo(b) { return a + b }; console.log(foo(1));"];
    let fail = vec!["let a = 1; function foo(b) { let a = 1; return a + b }; console.log(foo(1));"];

    Tester::new_without_config(NoUnusedVars::NAME, pass, fail).test();
}

#[test]
fn test_var_ignored() {
    let local = Some(json!(["local"]));
    let ignore_underscore = Some(json!([{ "vars": "all", "varsIgnorePattern": "^_" }]));
    let pass = vec![
        // does const count?
        ("var a", local),
        ("var _a", ignore_underscore.clone()),
        ("var a = 1; var _b = a", ignore_underscore.clone()),
    ];
    let fail = vec![
        ("var a_", ignore_underscore),
        // ("let a = 1;", None),
        // ("let a = 1; a += 2;", None)
    ];

    Tester::new(NoUnusedVars::NAME, pass, fail).test();
}

#[test]
fn test_var_read_write() {
    let pass = vec![
        "let a = 1; let b = a + 1; console.log(b)", // todo
        "var a = 1; let b = a++; console.log(b)",
    ];
    let fail = vec![
        // "let a = 1; a += 1;",
        // todo
        // "let a = 1; a = a + 1;",
    ];
    Tester::new_without_config(NoUnusedVars::NAME, pass, fail).test();
}

#[test]
fn test_spread_arr_simple() {
    let pass = vec![("let [b] = a; console.log(b);", None)];
    let fail = vec![("let [b] = a", None), ("let [b, c] = a; console.log(b);", None)];

    Tester::new(NoUnusedVars::NAME, pass, fail).test();
}

#[test]
fn test_spread_arr_ignored() {
    let ignore_underscore = Some(json!([{ "destructuredArrayIgnorePattern": "^_" }]));
    let ignore_rest_siblings = Some(json!([{ "ignoreRestSiblings": true }]));
    let pass = vec![
        ("let [_b] = a;", ignore_underscore.clone()),
        ("var { a, ...rest } = arr; console.log(rest)", ignore_rest_siblings),
        ("const [ a, _b, c ] = items;\nconsole.log(a+c);", ignore_underscore.clone()),
        ("const [ a, [_b], c ] = items;\nconsole.log(a+c);", ignore_underscore.clone()),
        ("const [ a, { b: [_b] }, c ] = items;\nconsole.log(a+c);", ignore_underscore),
    ];
    let fail = vec![("var { a, ...rest } = arr; console.log(rest)", None)];

    Tester::new(NoUnusedVars::NAME, pass, fail).test();
}

#[test]
fn test_spread_obj_simple() {
    let pass = vec!["let { a } = x; console.log(a)", "let { a: { b } } = x; console.log(b)"];
    let fail = vec![
        "let { a } = x;",
        "let { a: { b } } = x;",
        "let { a: { b, c, d } } = x; console.log(b + d)",
    ];
    Tester::new_without_config(NoUnusedVars::NAME, pass, fail).test();
}

#[test]
fn test_spread_compound() {
    let pass = vec![
        ("let { a: [b, c, { e }] } = x; console.log(b + c + e)", None),
        ("function foo({ bar = 1 }) { return bar }; foo()", None),
    ];
    let fail = vec![
        ("let { a: [b, c, d] } = x", None),
        ("export function fn2({ x, y }) {\n console.log(x); \n};", None),
    ];
    Tester::new(NoUnusedVars::NAME, pass, fail).test();
}

#[test]
fn test_catch_clause_simple() {
    let all = Some(json!([{"caughtErrors": "all"}]));
    let allow_underscore =
        Some(json!([{ "caughtErrors": "all", "caughtErrorsIgnorePattern": "^_" }]));
    let pass = vec![
        ("try {} catch (e) { }", None),
        ("try {} catch (_e) { }", allow_underscore.clone()),
        ("try {} catch (e) { console.error(e); }", all.clone()),
    ];
    let fail = vec![("try {} catch (e) { }", all), ("try {} catch (e) { }", allow_underscore)];
    Tester::new(NoUnusedVars::NAME, pass, fail).test();
}
