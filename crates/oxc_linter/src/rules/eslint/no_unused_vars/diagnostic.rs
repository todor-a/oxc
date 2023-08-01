use oxc_diagnostics::{
    miette::{self, Diagnostic},
    thiserror::Error,
};
use oxc_span::{Atom, Span};

const DECLARED_STR: &Atom = &Atom::new_inline("declared");
const ASSIGNED_STR: &Atom = &Atom::new_inline("assigned");
const IMPORTED_STR: &Atom = &Atom::new_inline("imported");
const EMPTY_STR: &Atom = &Atom::new_inline("");

#[derive(Debug, Error, Diagnostic)]
#[error("eslint(no-unused-vars): Unused variables are not allowed")]
#[diagnostic(severity(warning), help("{0} is {1} but never used{2}"))]
pub struct NoUnusedVarsDiagnostic(
    /* varName*/ pub Atom,
    /* action */ pub &'static Atom,
    /* additional */ pub Atom,
    #[label] pub Span,
);

impl NoUnusedVarsDiagnostic {
    /// Diagnostic for unused declaration, with no additional message.
    pub fn decl(var: Atom, span: Span) -> Self {
        Self(var, DECLARED_STR, EMPTY_STR.clone(), span)
    }

    /// Diagnostic for a variable that is declared and assigned a value, with no
    /// additional message.
    pub fn assigned(var: Atom, span: Span) -> Self {
        Self(var, ASSIGNED_STR, EMPTY_STR.clone(), span)
    }

    pub fn import(var: Atom, span: Span) -> Self {
        Self(var, IMPORTED_STR, EMPTY_STR.clone(), span)
    }

    pub fn with_additional(mut self, additional: Atom) -> Self {
        self.2 = additional;
        self
    }
}
