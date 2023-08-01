mod diagnostic;
mod ignored;
mod options;
#[cfg(test)]
mod tests;
mod usage;

pub use diagnostic::*;
pub use ignored::*;
pub use options::*;

#[allow(clippy::wildcard_imports)]
use oxc_ast::{ast::*, AstKind};
use oxc_macros::declare_oxc_lint;
use oxc_semantic::{ScopeFlags, SymbolId};
use oxc_span::{GetSpan, Span};
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
        println!("checking symbol '{}'", ctx.name());

        // order matters. We want to call cheap/high "yield" functions first.
        if ctx.is_exported() || self.is_ignored(&ctx) || dbg!(ctx.has_usages()) {
            return;
        }

        let name = ctx.name();
        match ctx.declaration().kind() {
            AstKind::ModuleDeclaration(module) => {
                debug_assert!(!module.is_export());
                // todo: should we try to find the span for the single import?
                ctx.diagnostic(NoUnusedVarsDiagnostic::import(name.clone(), module.span()));
            }
            AstKind::VariableDeclarator(decl) => {
                if decl.kind.is_var() && self.vars == VarsOption::Local {
                    return;
                }
                if let Some(UnusedBindingResult(span, false)) =
                    self.check_unused_binding_pattern(&ctx, &decl.id)
                {
                    ctx.diagnostic(NoUnusedVarsDiagnostic::decl(name.clone(), span));
                }
            }
            AstKind::Function(f) => {
                f.id.as_ref().map_or_else(
                    || debug_assert!(false, "Found unused function by symbol id but it is anonymous. This shouldn't be possible."),
                    |id| {
                        debug_assert!(&id.name == name, "Unused function with different name {} found while checking symbol named {name}", &id.name);
                        ctx.diagnostic(NoUnusedVarsDiagnostic::decl(name.clone(), id.span));
                    }
                );
            }
            AstKind::Class(class) => {
                class.id.as_ref().map_or_else(
                    || debug_assert!(false, "Found unused class by symbol id but it is anonymous. This shouldn't be possible."),
                    |id| {
                        debug_assert!(&id.name == name, "Unused class with different name {} found while checking symbol named {name}", &id.name);
                        ctx.diagnostic(NoUnusedVarsDiagnostic::decl(name.clone(), id.span));
                    }
                );
            }
            AstKind::CatchClause(catch) => {
                catch.param.as_ref().map_or_else(
                    || debug_assert!(false, "Found unused error in catch block by symbol id but it is anonymous. This shouldn't be possible."),
                    |id| {
                        // debug_assert!(&id.name == name, "Unused error in catch block with different name {} found while checking symbol named {name}", &id.name);
                        ctx.diagnostic(NoUnusedVarsDiagnostic::decl(name.clone(), id.span()));
                    }
                );
            }
            AstKind::FormalParameters(params) => self.check_unused_arguments(&ctx, params),
            s => debug_assert!(false, "handle decl kind {}", s.debug_name()),
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

#[derive(Debug, Clone, Copy, Default)]
struct UnusedBindingResult(Span, bool);
impl UnusedBindingResult {
    pub fn span(&self) -> Span {
        self.0
    }
    pub fn is_ignore(&self) -> bool {
        self.1
    }
    pub fn ignore(mut self) -> Self {
        self.1 = true;
        self
    }
    pub fn or(mut self, ignored: bool) -> Self {
        self.1 = self.1 || ignored;
        self
    }
}
impl From<Span> for UnusedBindingResult {
    fn from(value: Span) -> Self {
        Self(value, false)
    }
}
impl From<UnusedBindingResult> for Span {
    fn from(value: UnusedBindingResult) -> Self {
        value.0
    }
}

impl NoUnusedVars {
    fn check_unused_binding_pattern<'a>(
        &self,
        ctx: &SymbolContext<'_, 'a>,
        id: &BindingPattern<'a>,
    ) -> Option<UnusedBindingResult> {
        match &id.kind {
            BindingPatternKind::BindingIdentifier(id) => {
                if id.name == ctx.name() {
                    return Some(id.span.into());
                } else {
                    return None;
                }
            }
            BindingPatternKind::AssignmentPattern(id) => {
                return self.check_unused_binding_pattern(ctx, &id.left);
            }
            BindingPatternKind::ArrayPattern(arr) => {
                for el in arr.elements.iter() {
                    let Some(el) = el else { continue };
                    if let Some(id) = el.kind.identifier() {
                        if id.name != ctx.name() {
                            continue;
                        }
                        let ignored = self.is_ignored_array_destructured(&id.name);
                        return Some(UnusedBindingResult(id.span, ignored));
                    } else {
                        return self.check_unused_binding_pattern(ctx, el);
                    }
                }
                return None;
            }
            BindingPatternKind::ObjectPattern(obj) => {
                for el in obj.properties.iter() {
                    let maybe_res = self.check_unused_binding_pattern(ctx, &el.value);
                    if let Some(res) = maybe_res {
                        // has a rest sibling and the rule is configured to
                        // ignore variables that have them
                        let is_ignorable = self.ignore_rest_siblings && obj.rest.is_some();
                        return Some(res.or(is_ignorable));
                    }
                }
                return obj
                    .rest
                    .as_ref()
                    .map(|rest| self.check_unused_binding_pattern(ctx, &rest.argument))
                    .flatten();
            }
        }
    }

    fn check_unused_arguments<'a>(&self, ctx: &SymbolContext<'_, 'a>, args: &FormalParameters<'a>) {
        // short-circuit when not checking args or arg is inside a setter method
        // (setters always need params, even if unused)
        if self.args == ArgsOption::None
            || matches!(
                ctx.nth_parent(1).map(|n| n.kind()),
                Some(
                    AstKind::MethodDefinition(MethodDefinition {
                        kind: MethodDefinitionKind::Set,
                        ..
                    }) | AstKind::ObjectProperty(ObjectProperty { kind: PropertyKind::Set, .. })
                )
            )
        {
            return;
        }
        match self.args {
            ArgsOption::All => {
                // skip ignored or used args
                if self.is_ignored_arg(ctx.name()) || ctx.has_usages() {
                    return;
                }

                #[allow(clippy::collection_is_never_read)]
                args.items
                    .iter()
                    .map(|arg| self.check_unused_binding_pattern(ctx, &arg.pattern))
                    .find(Option::is_some)
                    .flatten()
                    .map_or_else(
                        || {},
                        |UnusedBindingResult(span, ignored)| {
                            if !ignored {
                                ctx.diagnostic(NoUnusedVarsDiagnostic::decl(
                                    ctx.name().clone(),
                                    span,
                                ));
                            }
                        },
                    );
            }
            ArgsOption::AfterUsed => {
                if self.is_ignored_arg(ctx.name()) || ctx.has_usages() {
                    return;
                }

                // set to true when a arg defined before the current one is
                // found to be used
                let mut has_prev_used = false;

                let scope_id = ctx.scope_id();
                let symbol_id = ctx.symbol_id();
                for arg in args.items.iter().rev() {
                    if has_prev_used {
                        break;
                    }
                    println!("checking {:?} for previous usages", arg.pattern.kind.identifier());

                    let Some(binding) = arg.pattern.kind.identifier() else { continue };
                    let Some(arg_symbol_id) = ctx.scopes().get_binding(scope_id, &binding.name) else { continue };

                    // we've reached the current argument, break
                    if arg_symbol_id == symbol_id {
                        break;
                    }

                    if ctx.has_usages_of(arg_symbol_id) {
                        has_prev_used = true;
                    }
                }

                if !has_prev_used {
                    for arg in args.items.iter() {
                        if let Some(UnusedBindingResult(arg_span, false)) =
                            self.check_unused_binding_pattern(ctx, &arg.pattern)
                        {
                            ctx.diagnostic(NoUnusedVarsDiagnostic::decl(
                                ctx.name().clone(),
                                arg_span,
                            ));
                            return;
                        }
                    }
                }
            }
            ArgsOption::None => {
                unreachable!();
            }
        };
    }
}
