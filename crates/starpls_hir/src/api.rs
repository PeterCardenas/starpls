use crate::{
    def::{Function as HirDefFunction, Stmt},
    module, source_map,
    typeck::{
        builtins::BuiltinFunction,
        intrinsics::{IntrinsicFunction, IntrinsicFunctionParam},
        Substitution, Ty,
    },
    Db, DisplayWithDb, Name, TyKind,
};
use starpls_common::File;
use starpls_syntax::ast::{self, AstNode, AstPtr};
use std::iter;

pub use crate::typeck::{Field, Param};

pub struct Semantics<'a> {
    db: &'a dyn Db,
}

impl<'a> Semantics<'a> {
    pub fn new(db: &'a dyn Db) -> Self {
        Self { db }
    }

    pub fn function_for_def(&self, file: File, stmt: ast::DefStmt) -> Option<Function> {
        let ptr = AstPtr::new(&ast::Statement::cast(stmt.syntax().clone())?);
        let stmt = source_map(self.db, file).stmt_map.get(&ptr)?;
        match &module(self.db, file)[*stmt] {
            Stmt::Def { func, .. } => Some((*func).into()),
            _ => None,
        }
    }

    pub fn resolve_call_expr(&self, file: File, expr: &ast::CallExpr) -> Option<Function> {
        let ty = self.type_of_expr(file, &expr.callee()?)?;
        Some(match ty.ty.kind() {
            TyKind::Function(func) => (*func).into(),
            TyKind::IntrinsicFunction(func, _) => (*func).into(),
            TyKind::BuiltinFunction(func) => (*func).into(),
            _ => return None,
        })
    }

    pub fn type_of_expr(&self, file: File, expr: &ast::Expression) -> Option<Type> {
        let ptr = AstPtr::new(expr);
        let expr = source_map(self.db, file).expr_map.get(&ptr)?;
        Some(self.db.infer_expr(file, *expr).into())
    }

    pub fn type_of_param(&self, file: File, param: &ast::Parameter) -> Option<Type> {
        let ptr = AstPtr::new(param);
        let param = source_map(self.db, file).param_map.get(&ptr)?;
        Some(self.db.infer_param(file, *param).into())
    }
}

pub struct Type {
    ty: Ty,
}

impl Type {
    pub fn is_function(&self) -> bool {
        matches!(
            self.ty.kind(),
            TyKind::Function(_) | TyKind::BuiltinFunction(_) | TyKind::IntrinsicFunction(_, _)
        )
    }

    pub fn is_user_defined_function(&self) -> bool {
        matches!(self.ty.kind(), TyKind::Function(_))
    }

    pub fn params(&self, db: &dyn Db) -> Vec<Param> {
        match self.ty.params(db) {
            Some(params) => params.collect(),
            None => Vec::new(),
        }
    }
}

impl Type {
    pub fn fields(&self, db: &dyn Db) -> Vec<(Field, Type)> {
        let fields = match self.ty.fields(db) {
            Some(fields) => fields,
            None => return Vec::new(),
        };

        fields.map(|(name, ty)| (name, ty.into())).collect()
    }

    pub fn doc(&self, db: &dyn Db) -> String {
        if let TyKind::BuiltinFunction(func) = self.ty.kind() {
            func.doc(db).clone()
        } else {
            String::new()
        }
    }
}

impl From<Ty> for Type {
    fn from(ty: Ty) -> Self {
        Self { ty }
    }
}

impl DisplayWithDb for Type {
    fn fmt(&self, db: &dyn Db, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.ty.fmt(db, f)
    }

    fn fmt_alt(&self, db: &dyn Db, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.ty.fmt_alt(db, f)
    }
}

pub struct Function(FunctionInner);

impl Function {
    pub fn name(&self, db: &dyn Db) -> Name {
        match self.0 {
            FunctionInner::HirDef(func) => func.name(db),
            FunctionInner::IntrinsicFunction(func) => func.name(db),
            FunctionInner::BuiltinFunction(func) => func.name(db),
        }
    }

    pub fn params(&self, db: &dyn Db) -> Vec<Param> {
        self.ty(db).params(db)
    }

    pub fn ty(&self, db: &dyn Db) -> Type {
        match self.0 {
            FunctionInner::HirDef(func) => TyKind::Function(func).intern(),
            FunctionInner::IntrinsicFunction(func) => {
                // TODO(withered-magic): Probably a terrible hack for creating the substitution here.
                let num_vars = func
                    .params(db)
                    .iter()
                    .filter_map(|param| match param {
                        IntrinsicFunctionParam::Positional { ty, .. }
                        | IntrinsicFunctionParam::Keyword { ty, .. }
                        | IntrinsicFunctionParam::ArgsList { ty } => Some(ty.clone()),
                        IntrinsicFunctionParam::KwargsDict => None,
                    })
                    .chain(iter::once(func.ret_ty(db)))
                    .map(|ty| match ty.kind() {
                        TyKind::BoundVar(index) => *index,
                        _ => 0,
                    })
                    .max()
                    .unwrap_or(0);
                TyKind::IntrinsicFunction(func, Substitution::new_identity(num_vars)).intern()
            }
            FunctionInner::BuiltinFunction(func) => TyKind::BuiltinFunction(func).intern(),
        }
        .into()
    }
}

impl From<HirDefFunction> for Function {
    fn from(func: HirDefFunction) -> Self {
        Self(FunctionInner::HirDef(func))
    }
}

impl From<IntrinsicFunction> for Function {
    fn from(func: IntrinsicFunction) -> Self {
        Self(FunctionInner::IntrinsicFunction(func))
    }
}

impl From<BuiltinFunction> for Function {
    fn from(func: BuiltinFunction) -> Self {
        Self(FunctionInner::BuiltinFunction(func))
    }
}

enum FunctionInner {
    HirDef(HirDefFunction),
    IntrinsicFunction(IntrinsicFunction),
    BuiltinFunction(BuiltinFunction),
}
