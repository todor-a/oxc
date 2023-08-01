use std::rc::Rc;

use oxc_ast::{ast::*, AstKind};
#[allow(clippy::wildcard_imports)]
use oxc_semantic::Reference;
use oxc_semantic::{
    AstNode, AstNodes, ScopeFlags, ScopeId, ScopeTree, Semantic, SymbolFlags, SymbolId, SymbolTable,
};
use oxc_span::{Atom, Span};

use crate::LintContext;

#[allow(clippy::wildcard_imports)]
use super::NoUnusedVars;

pub(crate) struct SymbolContext<'ctx, 'a> {
    ctx: &'ctx LintContext<'a>,
    name: &'ctx Atom,
    symbol_id: SymbolId,
    symbol_flags: SymbolFlags,
    scope_id: ScopeId,
    scope_flags: ScopeFlags,
    declaration: &'ctx AstNode<'a>,
}

impl<'ctx, 'a> SymbolContext<'ctx, 'a> {
    pub(crate) fn new(symbol_id: SymbolId, ctx: &'ctx LintContext<'a>) -> Self {
        let name = ctx.symbols().get_name(symbol_id);
        let declaration: &AstNode<'_> = ctx.semantic().symbol_declaration(symbol_id);
        let symbol_flags = ctx.symbols().get_flag(symbol_id);
        let scope_id = declaration.scope_id();
        let scope_flags = ctx.scopes().get_flags(scope_id);

        Self { ctx, name, symbol_id, symbol_flags, scope_id, scope_flags, declaration }
    }
    pub fn name(&self) -> &Atom {
        self.name
    }
    pub fn declaration(&self) -> &AstNode<'a> {
        self.declaration
    }

    pub fn is_exported(&self) -> bool {
        self.is_root()
            && (self.symbol_flags.contains(SymbolFlags::Export)
                || self.ctx.semantic().module_record().exported_bindings.contains_key(self.name))
    }

    pub const fn is_root(&self) -> bool {
        self.scope_flags.contains(ScopeFlags::Top)
        // self.scope_id.is_root()
    }

    pub fn has_usages(&self) -> bool {
        let can_skip_self_reassignment_check = self.symbol_flags.intersects(SymbolFlags::Class | SymbolFlags::ImportBinding | SymbolFlags::Type);
        let can_skip_self_call_check = self.symbol_flags.intersects(SymbolFlags::ImportBinding | SymbolFlags::CatchVariable | SymbolFlags::Type);
        self.ctx.symbols()
            .get_resolved_references(self.symbol_id)
            .filter(|r| r.is_read())
            .filter(|r| can_skip_self_reassignment_check || !self.is_self_reassignment(r))
            .filter(|r| can_skip_self_call_check || !self.is_self_call(r))
            .count()
            > 0
    }

    fn is_self_reassignment(&self, reference: &Reference) -> bool {
        false
    }
    fn is_self_call(&self, reference: &Reference) -> bool {
        false
    }
}

