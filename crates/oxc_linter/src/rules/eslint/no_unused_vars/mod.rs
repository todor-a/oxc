mod diagnostic;
mod ignored;
mod options;
mod usage;
#[cfg(test)]
mod tests;

pub use diagnostic::*;
pub use ignored::*;
pub use options::*;

#[allow(clippy::wildcard_imports)]
use oxc_ast::{ast::*, AstKind};
use oxc_macros::declare_oxc_lint;
use oxc_semantic::{ScopeFlags, SymbolId};
use regex::Regex;

use crate::{context::LintContext, rule::Rule};

use self::usage::SymbolContext;

/// Detects and reports variables, imports, args, functions, etc. that are defined and/or
/// assigned a value, but otherwise not used.
///
/// see: [ESLint - no-unused-vars](https://eslint.org/docs/latest/rules/no-unused-vars)
#[derive(Debug, Default, Clone)]
pub struct NoUnusedVars {
    /// Controls how usage of a variable in the global scope is checked.
    ///
    /// This option has two settings:
    /// 1. `all` checks all variables for usage, including those in the global
    ///    scope. This is the default setting.
    /// 2. `local` checks only that locally-declared variables are used but will
    ///    allow global variables to be unused.
    vars: VarsOption,
    /// Specifies exceptions to this rule for unused variables. Variables whose
    /// names match this pattern will be ignored.
    ///
    /// ## Example
    ///
    /// Examples of **correct** code for this option when the pattern is `^_`:
    /// ```javascript
    /// var _a = 10;
    /// var b = 10;
    /// console.log(b);
    /// ```
    vars_ignore_pattern: Option<Regex>,
    /// Controls how unused arguments are checked.
    ///
    /// This option has three settings:
    /// 1. `after-used` - Unused positional arguments that occur before the last
    ///    used argument will not be checked, but all named arguments and all
    ///    positional arguments after the last used argument will be checked.
    /// 2. `all` - All named arguments must be used.
    /// 3. `none` - Do not check arguments.
    args: ArgsOption,
    /// Specifies exceptions to this rule for unused arguments. Arguments whose
    /// names match this pattern will be ignored.
    ///
    /// ## Example
    ///
    /// Examples of **correct** code for this option when the pattern is `^_`:
    ///
    /// ```javascript
    /// function foo(_a, b) {
    ///    console.log(b);
    /// }
    /// foo(1, 2);
    /// ```
    args_ignore_pattern: Option<Regex>,
    /// Used for `catch` block validation.
    /// It has two settings:
    /// * `none` - do not check error objects. This is the default setting
    /// * `all` - all named arguments must be used`
    ///
    #[doc(hidden)]
    /// `none` corresponds to `false`, while `all` corresponds to `true`.
    caught_errors: bool,
    /// Specifies exceptions to this rule for errors caught within a `catch` block.
    /// Variables declared within a `catch` block whose names match this pattern
    /// will be ignored.
    ///
    /// ## Example
    ///
    /// Examples of **correct** code when the pattern is `^ignore`:
    ///
    /// ```javascript
    /// try {
    ///   // ...
    /// } catch (ignoreErr) {
    ///   console.error("Error caught in catch block");
    /// }
    /// ```
    caught_errors_ignore_pattern: Option<Regex>,
    /// This option specifies exceptions within destructuring patterns that will
    /// not be checked for usage. Variables declared within array destructuring
    /// whose names match this pattern will be ignored.
    ///
    /// ## Example
    ///
    /// Examples of **correct** code for this option, when the pattern is `^_`:
    /// ```javascript
    /// const [a, _b, c] = ["a", "b", "c"];
    /// console.log(a + c);
    ///
    /// const { x: [_a, foo] } = bar;
    /// console.log(foo);
    ///
    /// let _m, n;
    /// foo.forEach(item => {
    ///     [_m, n] = item;
    ///     console.log(n);
    /// });
    /// ```
    destructured_array_ignore_pattern: Option<Regex>,
    /// Using a Rest property it is possible to "omit" properties from an
    /// object, but by default the sibling properties are marked as "unused".
    /// With this option enabled the rest property's siblings are ignored.
    ///
    /// ## Example
    /// Examples of **correct** code when this option is set to `true`:
    /// ```js
    /// // 'foo' and 'bar' were ignored because they have a rest property sibling.
    /// var { foo, ...coords } = data;
    ///
    /// var bar;
    /// ({ bar, ...coords } = data);
    /// ```
    ignore_rest_siblings: bool,
}

declare_oxc_lint!(
    /// ### What it does
    ///
    /// Disallows variable declarations or imports that are not used in code.
    ///
    /// ### Why is this bad?
    ///
    /// Variables that are declared and not used anywhere in the code are most
    /// likely an error due to incomplete refactoring. Such variables take up
    /// space in the code and can lead to confusion by readers.
    ///
    /// A variable `foo` is considered to be used if any of the following are
    /// true:
    ///
    /// * It is called (`foo()`) or constructed (`new foo()`)
    /// * It is read (`var bar = foo`)
    /// * It is passed into a function as an argument (`doSomething(foo)`)
    /// * It is read inside of a function that is passed to another function
    ///   (`doSomething(function() { foo(); })`)
    ///
    /// A variable is _not_ considered to be used if it is only ever declared
    /// (`var foo = 5`) or assigned to (`foo = 7`).
    ///
    /// #### Exported
    ///
    /// In environments outside of CommonJS or ECMAScript modules, you may use
    /// `var` to create a global variable that may be used by other scripts. You
    /// can use the `/* exported variableName */` comment block to indicate that
    /// this variable is being exported and therefore should not be considered
    /// unused.
    ///
    /// Note that `/* exported */` has no effect for any of the following:
    /// * when the environment is `node` or `commonjs`
    /// * when `parserOptions.sourceType` is `module`
    /// * when `ecmaFeatures.globalReturn` is `true`
    ///
    /// The line comment `//exported variableName` will not work as `exported`
    /// is not line-specific.
    ///
    /// ### Example
    ///
    /// Examples of **incorrect** code for this rule:
    ///
    /// ```javascript
    /// /*eslint no-unused-vars: "error"*/
    /// /*global some_unused_var*/
    ///
    /// // It checks variables you have defined as global
    /// some_unused_var = 42;
    ///
    /// var x;
    ///
    /// // Write-only variables are not considered as used.
    /// var y = 10;
    /// y = 5;
    ///
    /// // A read for a modification of itself is not considered as used.
    /// var z = 0;
    /// z = z + 1;
    ///
    /// // By default, unused arguments cause warnings.
    /// (function(foo) {
    ///     return 5;
    /// })();
    ///
    /// // Unused recursive functions also cause warnings.
    /// function fact(n) {
    ///     if (n < 2) return 1;
    ///     return n * fact(n - 1);
    /// }
    ///
    /// // When a function definition destructures an array, unused entries from
    /// // the array also cause warnings.
    /// function getY([x, y]) {
    ///     return y;
    /// }
    /// ```
    ///
    /// Examples of **correct** code for this rule:
    /// ```javascript
    /// /*eslint no-unused-vars: "error"*/
    ///
    /// var x = 10;
    /// alert(x);
    ///
    /// // foo is considered used here
    /// myFunc(function foo() {
    ///     // ...
    /// }.bind(this));
    ///
    /// (function(foo) {
    ///     return foo;
    /// })();
    ///
    /// var myFunc;
    /// myFunc = setTimeout(function() {
    ///     // myFunc is considered used
    ///     myFunc();
    /// }, 50);
    ///
    /// // Only the second argument from the destructured array is used.
    /// function getY([, y]) {
    ///     return y;
    /// }
    /// ```
    ///
    /// Examples of **correct** code for `/* exported variableName */` operation:
    /// ```javascript
    /// /* exported global_var */
    ///
    /// var global_var = 42;
    /// ```
    NoUnusedVars,
    correctness
);

impl Rule for NoUnusedVars {
    fn from_configuration(value: serde_json::Value) -> Self {
        Self::from(value)
    }

    fn run_on_symbol(&self, symbol_id: SymbolId, lint_ctx: &LintContext<'_>) {
        let ctx = SymbolContext::new(symbol_id, lint_ctx);

        // order matters. We want to call cheap/high "yield" functions first.
        if ctx.is_exported() || self.is_ignored(&ctx) || ctx.has_usages() {
            return
        }

        // match ctx.declaration().kind() {
        //     AstKind::ModuleDeclaration(decl) => {
        //         self.check_unused_module_declaration(&ctx, decl);
        //     }
        //     AstKind::VariableDeclarator(decl) => {
        //         self.check_unused_variable_declarator(&ctx, decl);
        //     }
        //     AstKind::Function(f) => self.check_unused_function(&ctx, f),
        //     AstKind::Class(class) => self.check_unused_class(&ctx, class),
        //     AstKind::CatchClause(catch) => self.check_unused_catch_clause(&ctx, catch),
        //     AstKind::FormalParameters(params) => {
        //         self.check_unused_arguments(&ctx, params);
        //     }
        //     s => debug_assert!(false, "handle decl kind {}", s.debug_name()),
        // };

    }

}
