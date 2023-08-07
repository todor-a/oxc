use oxc_ast::{ast::*, AstKind};
#[allow(clippy::wildcard_imports)]
use oxc_semantic::Reference;
use oxc_semantic::{
    AstNode, AstNodes, ScopeFlags, ScopeId, ScopeTree, SymbolFlags, SymbolId, SymbolTable,
};
use oxc_span::{Atom, Span};

use crate::LintContext;

pub(crate) struct SymbolContext<'ctx, 'a> {
    ctx: &'ctx LintContext<'a>,
    name: &'ctx Atom,
    symbol_id: SymbolId,
    symbol_flags: SymbolFlags,
    /// Scope id of symbol declaration
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

    pub fn scope_id(&self) -> ScopeId {
        self.scope_id
    }

    pub fn symbol_id(&self) -> SymbolId {
        self.symbol_id
    }

    /// Get the nth parent above this symbol's declaration. 0-indexed.
    pub fn nth_parent(&self, n: usize) -> Option<&AstNode<'a>> {
        self.nodes().iter_parents(self.declaration.id()).nth(n + 1)
    }

    pub fn diagnostic<T: Into<oxc_diagnostics::Error>>(&self, diagnostic: T) {
        self.ctx.diagnostic(diagnostic);
    }

    pub fn nodes(&self) -> &AstNodes<'a> {
        self.ctx.nodes()
    }

    pub fn scopes(&self) -> &ScopeTree {
        self.ctx.scopes()
    }

    pub fn symbols(&self) -> &SymbolTable {
        self.ctx.symbols()
    }

    pub fn is_exported(&self) -> bool {
        (self.is_root()
            && (self.symbol_flags.contains(SymbolFlags::Export)
                || self.ctx.semantic().module_record().exported_bindings.contains_key(self.name)))
            || self.in_export_node()
    }
    fn in_export_node(&self) -> bool {
        for parent in self.ctx.nodes().iter_parents(self.declaration.id()).skip(1) {
            match parent.kind() {
                AstKind::ModuleDeclaration(module) => {
                    return module.is_export();
                }
                AstKind::VariableDeclaration(_) => {
                    continue;
                }
                _ => {
                    return false;
                }
            }
        }
        false
    }

    pub const fn is_root(&self) -> bool {
        // self.scope_flags.contains(ScopeFlags::Top)
        self.scope_id.is_root()
    }

    // pub fn has_usages(&self) -> bool {
    //     // let can_skip_self_reassignment_check = self.symbol_flags.intersects(SymbolFlags::Class | SymbolFlags::ImportBinding | SymbolFlags::Type);
    //     // let can_skip_self_call_check = self.symbol_flags.intersects(SymbolFlags::ImportBinding | SymbolFlags::CatchVariable | SymbolFlags::Type);

    //     let do_self_reassignment_check = self.symbol_flags.intersects(SymbolFlags::Variable);
    //     let do_self_call_check =
    //         !self.symbol_flags.intersects(SymbolFlags::ImportBinding | SymbolFlags::CatchVariable);

    //     self.ctx
    //         .symbols()
    //         .get_resolved_references(self.symbol_id)
    //         .filter(|r| r.is_read())
    //         .filter(|r| !do_self_reassignment_check || dbg!(!self.is_self_reassignment(r)))
    //         .filter(|r| !do_self_call_check || dbg!(!self.is_self_call(r)))
    //         // .filter(|r| !(do_self_reassignment_check && !self.is_self_reassignment(r)))
    //         // .filter(|r| !(do_self_call_check && !self.is_self_call(r)))
    //         .count()
    //         > 0
    // }
    pub fn has_usages(&self) -> bool {
        self.has_usages_of(self.symbol_id)
    }
    
    pub fn has_usages_of(&self, symbol_id: SymbolId) -> bool {
        let do_self_reassignment_check = self.symbol_flags.intersects(SymbolFlags::Variable);
        let do_self_call_check =
            !self.symbol_flags.intersects(SymbolFlags::ImportBinding | SymbolFlags::CatchVariable);

        let rs: Vec<_> =
            self.ctx.symbols().get_resolved_references(symbol_id).filter(|r| r.is_read()).collect();
        // for reference in
        //     self.ctx.symbols().get_resolved_references(self.symbol_id).filter(|r|
        //     r.is_read())
        for reference in rs {
            if do_self_reassignment_check && self.is_self_reassignment(reference) {
                continue;
            }
            if do_self_call_check && self.is_self_call(reference) {
                continue;
            }
            return true;
        }

        false
    }

    fn is_self_reassignment(&self, reference: &Reference) -> bool {
        let Some(symbol_id) = reference.symbol_id() else {
            debug_assert!(
                false,
                "is_self_update() should only be called on resolved symbol references"
            );
            return true;
        };
        let Some(node_id) = reference.ast_node_id() else {
            debug_assert!(
                false,
                "Reference to symbol {} is missing an ast node id", self.ctx.symbols().get_name(symbol_id)
            );
            return true;
        };
        let mut is_used_by_others = true;
        let name = self.name();
        for node in self.ctx.nodes().iter_parents(node_id).skip(1) {
            println!("kind: {}, used: {is_used_by_others}", node.kind().debug_name());
            match node.kind() {
                // references used in declaration of another variable are definitely
                // used by others
                AstKind::VariableDeclarator(v) => {
                    debug_assert!(
                        v.id.kind.identifier().map_or_else(|| true, |id| id.name != name),
                        "While traversing {name}'s reference's parent nodes, found {name}'s declaration. This algorithm assumes that variable declarations do not appear in references."
                    );
                    // definitely used, short-circuit
                    return false;
                }
                // When symbol is being assigned a new value, we flag the reference
                // as only affecting itself until proven otherwise.
                AstKind::UpdateExpression(_) | AstKind::SimpleAssignmentTarget(_) => {
                    is_used_by_others = false;
                }
                // RHS usage when LHS != reference's symbol is definitely used by
                // others
                AstKind::AssignmentExpression(AssignmentExpression {
                    left: AssignmentTarget::SimpleAssignmentTarget(target),
                    ..
                }) => {
                    match target {
                        SimpleAssignmentTarget::AssignmentTargetIdentifier(id) => {
                            if id.name == name {
                                is_used_by_others = false;
                            } else {
                                return false; // we can short-circuit
                            }
                        }
                        // variable is being used to index another variable, this is
                        // always a usage
                        // todo: check self index?
                        SimpleAssignmentTarget::MemberAssignmentTarget(_) => return false,
                        _ => {
                            // debug_assert!(false, "is_self_update only supports AssignmentTargetIdentifiers right now. Please update this function. Found {t:#?}",);
                        }
                    }
                }
                AstKind::Argument(_) => {
                    break;
                }
                // expression is over, save cycles by breaking
                // todo: do we need to check if variable is used as iterator in loops?
                AstKind::ForInStatement(_)
                | AstKind::ForOfStatement(_)
                | AstKind::WhileStatement(_)
                | AstKind::Function(_)
                | AstKind::ExpressionStatement(_) => {
                    break;
                }
                AstKind::YieldExpression(_) => return false,
                _ => { /* continue up tree */ }
            }
        }

        !is_used_by_others
    }

    fn is_self_call(&self, reference: &Reference) -> bool {
        let scopes = self.ctx.scopes();

        // determine what scope the call occurred in
        let node_id = reference.node_id();
        let node = self
            .nodes()
            .iter_parents(node_id)
            .skip(1)
            .filter(|n| {
                dbg!(n.kind().debug_name());
                !matches!(n.kind(), AstKind::ParenthesizedExpression(_))
            })
            .nth(0);
        if !matches!(
            node.map(|n| {
                println!("{}", n.kind().debug_name());
                n.kind()
            }),
            Some(AstKind::CallExpression(_) | AstKind::NewExpression(_))
        ) {
            return false;
        }

        let call_scope_id = self.ctx.nodes().get_node(node_id).scope_id();
        // note: most nodes record what scope they were declared in. The
        // exception is functions and classes, which record the scopes they create.
        let decl_scope_id = self
            .scopes()
            .ancestors(self.scope_id)
            .find(|scope_id| self.scopes().get_binding(*scope_id, self.name()).is_some())
            .unwrap();
        if call_scope_id == decl_scope_id {
            return false;
        };

        let is_called_inside_self = scopes.ancestors(call_scope_id).any(|scope_id| {
            // let flags = scopes.get_flags(scope_id);
            // scope_id == decl_scope_id && flags.intersects(ScopeFlags::Function | ScopeFlags::Arrow)
            scope_id == decl_scope_id
        });

        return is_called_inside_self;
    }
}
