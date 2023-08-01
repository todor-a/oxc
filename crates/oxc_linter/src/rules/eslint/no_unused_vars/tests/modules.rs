use crate::tester::Tester;
use crate::{rule::RuleMeta, rules::eslint::no_unused_vars::NoUnusedVars};
use serde_json::json;

#[test]
fn test_modules_simple() {
    let pass = vec![
        ("export const foo = 1;", None),
        ("const foo = 1; export { foo }", None),
        ("const foo = 1; export default foo", None),
        ("export * as foo from './foo';", None),
        ("export default { a: true };", None),
        ("export function foo() {}", None),
        ("export default function foo() {}", None),
        ("export class Foo {}", None),
        ("export default class Foo {}", None),
        ("import { foo } from './foo'; foo();", None),
        ("import foo, { bar } from './foo'; foo(bar);", None),
        ("import { foo as bar } from './foo'; bar();", None),
        ("import { ignored } from './foo'", Some(json!([{ "varsIgnorePattern": "^ignored" }]))),
        ("import { foo as _foo } from './foo'", Some(json!([{ "varsIgnorePattern": "^_" }]))),
        ("import _foo from './foo'", Some(json!([{ "varsIgnorePattern": "^_" }]))),
        // todo
        // ("export const Foo = class Foo {}", None),
    ];
    let fail = vec![
        ("import { unused } from './foo';", None),
        ("import unused from './foo';", None),
        ("import unused, { foo } from './foo'; foo()", None),
        ("import * as unused from './foo';", None),
        ("import { foo as bar } from './foo'; foo();", None),
    ];
    Tester::new(NoUnusedVars::NAME, pass, fail).test();
}
