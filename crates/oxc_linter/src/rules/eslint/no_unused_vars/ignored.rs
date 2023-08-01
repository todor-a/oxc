use oxc_ast::AstKind;
use oxc_semantic::AstNode;
use oxc_span::Atom;
use regex::Regex;

use super::{NoUnusedVars, usage::SymbolContext};

impl NoUnusedVars {
    fn is_none_or_match(re: Option<&Regex>, haystack: &str) -> bool {
        re.map_or(false, |pat| pat.is_match(haystack))
    }

    pub(super) fn is_ignored_var(&self, name: &str) -> bool {
        Self::is_none_or_match(self.vars_ignore_pattern.as_ref(), name)
    }

    pub(super) fn is_ignored_arg(&self, name: &str) -> bool {
        Self::is_none_or_match(self.args_ignore_pattern.as_ref(), name)
    }

    pub(super) fn is_ignored_array_destructured(&self, name: &str) -> bool {
        Self::is_none_or_match(self.destructured_array_ignore_pattern.as_ref(), name)
    }

    pub(super) fn is_ignored_catch_err(&self, name: &str) -> bool {
        !self.caught_errors
            || Self::is_none_or_match(self.caught_errors_ignore_pattern.as_ref(), name)
    }

    pub(super) fn is_ignored<'ctx>(
        &self,
        ctx: &'ctx SymbolContext<'ctx, '_>
    ) -> bool {
        let declared_binding: &str = ctx.name();
        match ctx.declaration().kind() {
            AstKind::VariableDeclarator(_)
            | AstKind::Function(_)
            | AstKind::Class(_)
            | AstKind::ModuleDeclaration(_) => self.is_ignored_var(declared_binding),
            AstKind::CatchClause(_) => self.is_ignored_catch_err(declared_binding),
            AstKind::FormalParameters(_) | AstKind::FormalParameter(_) => {
                self.is_ignored_arg(declared_binding)
            }
            s => {
                // panic when running test cases so we can find unsupported node kinds
                debug_assert!(
                    false,
                    "is_ignored_decl did not know how to handle node of kind {}",
                    s.debug_name()
                );
                false
            }
        }
    }
}

#[test]
fn test_ignored() {
    use oxc_span::Atom;
    let rule = NoUnusedVars::from(serde_json::json!([
        {
            "varsIgnorePattern": "^_",
            "argsIgnorePattern": "[iI]gnored",
            "caughtErrorsIgnorePattern": "err.*",
            "caughtErrors": "all",
            "destructuredArrayIgnorePattern": "^_",
        }
    ]));

    assert!(rule.is_ignored_var("_x"));
    assert!(rule.is_ignored_var(&Atom::from("_x")));
    assert!(!rule.is_ignored_var("notIgnored"));

    assert!(rule.is_ignored_arg("ignored"));
    assert!(rule.is_ignored_arg("alsoIgnored"));
    assert!(rule.is_ignored_arg(&Atom::from("ignored")));
    assert!(rule.is_ignored_arg(&Atom::from("alsoIgnored")));

    assert!(rule.is_ignored_catch_err("err"));
    assert!(rule.is_ignored_catch_err("error"));
    assert!(!rule.is_ignored_catch_err("e"));

    assert!(rule.is_ignored_array_destructured("_x"));
    assert!(rule.is_ignored_array_destructured(&Atom::from("_x")));
    assert!(!rule.is_ignored_array_destructured("notIgnored"));
}
