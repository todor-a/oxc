use oxc_ast::{ast::*, AstKind};
#[allow(clippy::wildcard_imports)]
use oxc_semantic::Reference;
use oxc_semantic::{AstNode, ScopeFlags, ScopeId, SymbolFlags, SymbolId};
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
        // let can_skip_self_reassignment_check = self.symbol_flags.intersects(SymbolFlags::Class | SymbolFlags::ImportBinding | SymbolFlags::Type);
        // let can_skip_self_call_check = self.symbol_flags.intersects(SymbolFlags::ImportBinding | SymbolFlags::CatchVariable | SymbolFlags::Type);

        let do_self_reassignment_check = self.symbol_flags.intersects(SymbolFlags::Variable);
        let do_self_call_check =
            !self.symbol_flags.intersects(SymbolFlags::ImportBinding | SymbolFlags::CatchVariable);

        self.ctx
            .symbols()
            .get_resolved_references(self.symbol_id)
            .filter(|r| r.is_read())
            .filter(|r| !(do_self_reassignment_check && self.is_self_reassignment(r)))
            .filter(|r| !(do_self_call_check && self.is_self_call(r)))
            .count()
            > 0
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
                    return true;
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
                                return true; // we can short-circuit
                            }
                        }
                        // variable is being used to index another variable, this is
                        // always a usage
                        // todo: check self index?
                        SimpleAssignmentTarget::MemberAssignmentTarget(_) => {
                            return true;
                        }
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
                _ => { /* continue up tree */ }
            }
        }

        is_used_by_others
    }

    fn is_self_call(&self, reference: &Reference) -> bool {
        let scopes = self.ctx.scopes();

        // determine what scope the call occurred in
        let Some(node_id) = reference.ast_node_id() else { return true };
        let scope_id = self.ctx.nodes().get_node(node_id).scope_id();

        let is_called_inside_self = scopes.ancestors(scope_id).any(|scope_id| {
            scope_id == self.scope_id && scopes.get_flags(scope_id).intersects(ScopeFlags::Function)
        });

        return !is_called_inside_self;
    }
}
