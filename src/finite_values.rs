//! Bounded, static values from the explicitly selected source files.
use anyhow::{Context, Result};
use rayon::prelude::*;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use swc_common::{sync::Lrc, FileName, SourceMap};
use swc_ecma_ast::*;
use swc_ecma_parser::{lexer::Lexer, Parser, StringInput, Syntax, TsSyntax};

const MAX_DEPTH: usize = 8;
const MAX_VALUES: usize = 256;
const MAX_EXPANDED_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug)]
pub(crate) enum FiniteValue {
    Strings(Vec<String>),
    Array(Vec<String>),
    Object(HashMap<String, FiniteValue>),
}

impl FiniteValue {
    fn string(value: &str) -> Option<Self> {
        (value.len() <= MAX_EXPANDED_BYTES).then(|| Self::Strings(vec![value.to_string()]))
    }
    fn byte_len(&self) -> usize {
        match self {
            Self::Strings(values) | Self::Array(values) => values.iter().map(String::len).sum(),
            Self::Object(values) => values
                .iter()
                .map(|(key, value)| key.len() + value.byte_len())
                .sum(),
        }
    }

    pub(crate) fn strings(self) -> Option<Vec<String>> {
        match self {
            Self::Strings(values) => Some(values),
            Self::Object(_) | Self::Array(_) => None,
        }
    }

    pub(crate) fn iteration_values(self) -> Option<Self> {
        match self {
            Self::Array(values) => Some(Self::Strings(values)),
            _ => None,
        }
    }
}

#[derive(Clone)]
enum Definition {
    Type(Box<TsType>),
    Value(Option<Box<Expr>>, Option<Box<TsType>>),
}

#[derive(Default)]
struct ModuleValues {
    source: String,
    definitions: HashMap<(String, bool), Definition>,
    exports: HashSet<(String, bool)>,
    imports: HashMap<String, (String, String, bool)>,
    has_relative_imports: bool,
}

impl ModuleValues {
    fn from_module(source: String, module: &Module) -> Self {
        let mut values = Self {
            source,
            ..Self::default()
        };
        for item in &module.body {
            let (decl, exported) = match item {
                ModuleItem::Stmt(Stmt::Decl(decl)) => (Some(decl), false),
                ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export)) => {
                    (Some(&export.decl), true)
                }
                ModuleItem::ModuleDecl(ModuleDecl::Import(import)) => {
                    if let Some(source) = import
                        .src
                        .value
                        .as_str()
                        .filter(|source| source.starts_with('.'))
                    {
                        values.has_relative_imports = true;
                        for specifier in &import.specifiers {
                            if let ImportSpecifier::Named(named) = specifier {
                                let name = match &named.imported {
                                    Some(ModuleExportName::Ident(name)) => {
                                        Some(name.sym.to_string())
                                    }
                                    Some(ModuleExportName::Str(name)) => {
                                        name.value.as_str().map(str::to_string)
                                    }
                                    None => Some(named.local.sym.to_string()),
                                };
                                if let Some(name) = name {
                                    values.imports.insert(
                                        named.local.sym.to_string(),
                                        (
                                            source.to_string(),
                                            name,
                                            import.type_only || named.is_type_only,
                                        ),
                                    );
                                }
                            }
                        }
                    }
                    (None, false)
                }
                _ => (None, false),
            };
            match decl {
                Some(Decl::TsTypeAlias(alias)) => {
                    values.definitions.insert(
                        (alias.id.sym.to_string(), true),
                        Definition::Type(alias.type_ann.clone()),
                    );
                    if exported {
                        values.exports.insert((alias.id.sym.to_string(), true));
                    }
                }
                Some(Decl::Var(var)) => {
                    for decl in &var.decls {
                        if let Pat::Ident(binding) = &decl.name {
                            let name = binding.id.sym.to_string();
                            values.definitions.insert(
                                (name.clone(), false),
                                Definition::Value(
                                    if var.kind == VarDeclKind::Const {
                                        decl.init.clone()
                                    } else {
                                        None
                                    },
                                    binding.type_ann.as_ref().map(|ty| ty.type_ann.clone()),
                                ),
                            );
                            if exported {
                                values.exports.insert((name, false));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        values
    }
}

#[derive(Default)]
pub(crate) struct FiniteIndex {
    modules: HashMap<PathBuf, ModuleValues>,
}

pub(crate) fn normalized(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap().join(path)
    };
    let mut result = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            _ => result.push(component.as_os_str()),
        }
    }
    result
}

impl FiniteIndex {
    pub(crate) fn load(paths: &[PathBuf]) -> Result<Self> {
        let modules: Result<Vec<_>> = paths
            .par_iter()
            .map(|path| {
                let source = std::fs::read_to_string(path)
                    .with_context(|| format!("Failed to read file: {}", path.display()))?;
                let cm: Lrc<SourceMap> = Default::default();
                let file = cm.new_source_file(FileName::Real(path.clone()).into(), source.clone());
                let syntax = Syntax::Typescript(TsSyntax {
                    tsx: path
                        .extension()
                        .is_some_and(|ext| ext == "tsx" || ext == "jsx"),
                    decorators: true,
                    ..Default::default()
                });
                let lexer = Lexer::new(syntax, Default::default(), StringInput::from(&*file), None);
                let mut parser = Parser::new_from(lexer);
                let parsed = parser
                    .parse_module()
                    .ok()
                    .filter(|_| parser.take_errors().is_empty());
                let values = match parsed {
                    Some(module) => ModuleValues::from_module(source, &module),
                    None => ModuleValues {
                        source,
                        ..Default::default()
                    },
                };
                Ok((normalized(path), values))
            })
            .collect();
        Ok(Self {
            modules: modules?.into_iter().collect(),
        })
    }

    pub(crate) fn source(&self, path: &Path) -> Option<&str> {
        self.modules
            .get(&normalized(path))
            .map(|module| module.source.as_str())
    }

    pub(crate) fn has_dependencies(&self) -> bool {
        self.modules
            .values()
            .any(|module| module.has_relative_imports)
    }

    fn import_path(&self, path: &Path, source: &str) -> Option<PathBuf> {
        let base = normalized(&path.parent()?.join(source));
        let candidates = if base.extension().is_some() {
            vec![base]
        } else {
            ["ts", "tsx", "js", "jsx", "mts", "cts", "mjs", "cjs"]
                .iter()
                .flat_map(|ext| [base.with_extension(ext), base.join(format!("index.{ext}"))])
                .collect()
        };
        // Membership alone resolves imports; no existence checks or reads outside input.
        let matches: Vec<_> = candidates
            .into_iter()
            .filter(|path| self.modules.contains_key(path))
            .collect();
        (matches.len() == 1).then(|| matches[0].clone())
    }
}

#[derive(Clone)]
pub(crate) struct FiniteResolver {
    index: Arc<FiniteIndex>,
    path: PathBuf,
    pub(crate) bindings: HashMap<String, Option<FiniteValue>>,
    types: HashMap<String, Box<TsType>>,
}

impl FiniteResolver {
    pub(crate) fn new(path: &Path, module: &Module, index: Option<&Arc<FiniteIndex>>) -> Self {
        let path = normalized(path);
        let index = index.cloned().unwrap_or_else(|| {
            Arc::new(FiniteIndex {
                modules: HashMap::from([(
                    path.clone(),
                    ModuleValues::from_module(String::new(), module),
                )]),
            })
        });
        Self {
            index,
            path,
            bindings: HashMap::new(),
            types: HashMap::new(),
        }
    }

    pub(crate) fn capture(&mut self, binding: &BindingIdent, init: Option<&Expr>) {
        let value = if let Some(ty) = &binding.type_ann {
            self.ty(
                &ty.type_ann,
                &self.path,
                0,
                &mut HashSet::new(),
                &mut (MAX_VALUES * MAX_DEPTH),
            )
        } else {
            init.and_then(|expr| {
                self.initializer(
                    expr,
                    &self.path,
                    0,
                    &mut HashSet::new(),
                    &mut (MAX_VALUES * MAX_DEPTH),
                )
            })
        };
        self.bindings.insert(binding.id.sym.to_string(), value);
    }

    pub(crate) fn capture_type(&mut self, alias: &TsTypeAliasDecl) {
        self.types
            .insert(alias.id.sym.to_string(), alias.type_ann.clone());
    }

    pub(crate) fn expression(&self, expression: &Expr) -> Option<FiniteValue> {
        self.expr(
            expression,
            &self.path,
            0,
            &mut HashSet::new(),
            &mut (MAX_VALUES * MAX_DEPTH),
        )
    }

    fn symbol(
        &self,
        name: &str,
        type_only: bool,
        path: &Path,
        depth: usize,
        visited: &mut HashSet<(PathBuf, String, bool)>,
        budget: &mut usize,
    ) -> Option<FiniteValue> {
        if depth >= MAX_DEPTH || *budget == 0 {
            return None;
        }
        *budget -= 1;
        if path == self.path && !type_only {
            if let Some(value) = self.bindings.get(name) {
                return value.clone();
            }
        }
        let identity = (path.to_path_buf(), name.to_string(), type_only);
        if !visited.insert(identity.clone()) {
            return None;
        }
        let result = (|| {
            if path == self.path && type_only {
                if let Some(ty) = self.types.get(name) {
                    return self.ty(ty, path, depth + 1, visited, budget);
                }
            }
            let module = self.index.modules.get(path)?;
            if let Some((source, imported, import_type_only)) = module.imports.get(name) {
                if !type_only && *import_type_only {
                    return None;
                }
                let target = self.index.import_path(path, source)?;
                if !self
                    .index
                    .modules
                    .get(&target)?
                    .exports
                    .contains(&(imported.clone(), type_only))
                {
                    return None;
                }
                return self.symbol(imported, type_only, &target, depth + 1, visited, budget);
            }
            match module.definitions.get(&(name.to_string(), type_only))? {
                Definition::Type(ty) => self.ty(ty, path, depth + 1, visited, budget),
                Definition::Value(_, Some(ty)) => self.ty(ty, path, depth + 1, visited, budget),
                Definition::Value(Some(expr), None) => {
                    self.initializer(expr, path, depth + 1, visited, budget)
                }
                _ => None,
            }
        })();
        visited.remove(&identity);
        result
    }

    fn initializer(
        &self,
        expr: &Expr,
        path: &Path,
        depth: usize,
        visited: &mut HashSet<(PathBuf, String, bool)>,
        budget: &mut usize,
    ) -> Option<FiniteValue> {
        let value = self.expr(expr, path, depth, visited, budget)?;
        // `const` fixes the binding, not object properties or array elements.
        match &value {
            FiniteValue::Strings(_) => Some(value),
            _ if const_asserted(expr) => Some(value),
            _ => None,
        }
    }

    fn ty(
        &self,
        ty: &TsType,
        path: &Path,
        depth: usize,
        visited: &mut HashSet<(PathBuf, String, bool)>,
        budget: &mut usize,
    ) -> Option<FiniteValue> {
        if depth >= MAX_DEPTH || *budget == 0 {
            return None;
        }
        *budget -= 1;
        match ty {
            TsType::TsLitType(literal) => match &literal.lit {
                TsLit::Str(value) => FiniteValue::string(value.value.as_str()?),
                _ => None,
            },
            TsType::TsTypeRef(reference) => match &reference.type_name {
                TsEntityName::Ident(name) => {
                    self.symbol(name.sym.as_ref(), true, path, depth + 1, visited, budget)
                }
                _ => None,
            },
            TsType::TsParenthesizedType(ty) => {
                self.ty(&ty.type_ann, path, depth + 1, visited, budget)
            }
            TsType::TsUnionOrIntersectionType(TsUnionOrIntersectionType::TsUnionType(union)) => {
                let mut values = BTreeSet::new();
                for ty in &union.types {
                    values.extend(self.ty(ty, path, depth + 1, visited, budget)?.strings()?);
                    if values.len() > MAX_VALUES
                        || values.iter().map(String::len).sum::<usize>() > MAX_EXPANDED_BYTES
                    {
                        return None;
                    }
                }
                Some(FiniteValue::Strings(values.into_iter().collect()))
            }
            _ => None,
        }
    }

    fn expr(
        &self,
        expr: &Expr,
        path: &Path,
        depth: usize,
        visited: &mut HashSet<(PathBuf, String, bool)>,
        budget: &mut usize,
    ) -> Option<FiniteValue> {
        if depth >= MAX_DEPTH || *budget == 0 {
            return None;
        }
        *budget -= 1;
        match expr {
            Expr::Lit(Lit::Str(value)) => FiniteValue::string(value.value.as_str()?),
            Expr::Ident(name) => {
                self.symbol(name.sym.as_ref(), false, path, depth + 1, visited, budget)
            }
            Expr::Paren(expr) => self.expr(&expr.expr, path, depth + 1, visited, budget),
            Expr::TsAs(expr) => self.expr(&expr.expr, path, depth + 1, visited, budget),
            Expr::TsConstAssertion(expr) => self.expr(&expr.expr, path, depth + 1, visited, budget),
            Expr::TsTypeAssertion(expr) => self.expr(&expr.expr, path, depth + 1, visited, budget),
            Expr::Array(array) => {
                let mut values = BTreeSet::new();
                for item in &array.elems {
                    let item = item.as_ref()?;
                    if item.spread.is_some() {
                        return None;
                    }
                    values.extend(
                        self.expr(&item.expr, path, depth + 1, visited, budget)?
                            .strings()?,
                    );
                    if values.len() > MAX_VALUES
                        || values.iter().map(String::len).sum::<usize>() > MAX_EXPANDED_BYTES
                    {
                        return None;
                    }
                }
                Some(FiniteValue::Array(values.into_iter().collect()))
            }
            Expr::Object(object) => {
                let mut values = HashMap::new();
                for prop in &object.props {
                    let PropOrSpread::Prop(prop) = prop else {
                        return None;
                    };
                    let Prop::KeyValue(prop) = prop.as_ref() else {
                        return None;
                    };
                    let name = match &prop.key {
                        PropName::Ident(name) => name.sym.to_string(),
                        PropName::Str(name) => name.value.as_str()?.to_string(),
                        _ => return None,
                    };
                    values.insert(
                        name,
                        self.expr(&prop.value, path, depth + 1, visited, budget)?,
                    );
                    if values.len() > MAX_VALUES
                        || values
                            .iter()
                            .map(|(key, value)| key.len() + value.byte_len())
                            .sum::<usize>()
                            > MAX_EXPANDED_BYTES
                    {
                        return None;
                    }
                }
                Some(FiniteValue::Object(values))
            }
            Expr::Member(member) => {
                let FiniteValue::Object(values) =
                    self.expr(&member.obj, path, depth + 1, visited, budget)?
                else {
                    return None;
                };
                let name = match &member.prop {
                    MemberProp::Ident(name) => name.sym.to_string(),
                    MemberProp::Computed(expr) => {
                        let names = self
                            .expr(&expr.expr, path, depth + 1, visited, budget)?
                            .strings()?;
                        if names.len() != 1 {
                            return None;
                        }
                        names[0].clone()
                    }
                    _ => return None,
                };
                values.get(&name).cloned()
            }
            Expr::Tpl(template) => {
                let mut values = vec![String::new()];
                for (index, quasi) in template.quasis.iter().enumerate() {
                    let text = quasi.cooked.as_ref()?.as_str()?;
                    if values
                        .iter()
                        .map(|value| value.len() + text.len())
                        .sum::<usize>()
                        > MAX_EXPANDED_BYTES
                    {
                        return None;
                    }
                    for value in &mut values {
                        value.push_str(text);
                    }
                    if let Some(expr) = template.exprs.get(index) {
                        let substitutions = self
                            .expr(expr, path, depth + 1, visited, budget)?
                            .strings()?;
                        if values.len().checked_mul(substitutions.len())? > MAX_VALUES {
                            return None;
                        }
                        if values
                            .iter()
                            .flat_map(|prefix| {
                                substitutions
                                    .iter()
                                    .map(move |value| prefix.len() + value.len())
                            })
                            .sum::<usize>()
                            > MAX_EXPANDED_BYTES
                        {
                            return None;
                        }
                        values = values
                            .iter()
                            .flat_map(|prefix| {
                                substitutions
                                    .iter()
                                    .map(move |value| format!("{prefix}{value}"))
                            })
                            .collect();
                    }
                }
                Some(FiniteValue::Strings(values))
            }
            _ => None,
        }
    }
}

fn const_asserted(expr: &Expr) -> bool {
    match expr {
        Expr::Paren(expr) => const_asserted(&expr.expr),
        Expr::TsConstAssertion(_) => true,
        Expr::TsAs(expr) => matches!(expr.type_ann.as_ref(), TsType::TsTypeRef(reference)
            if matches!(&reference.type_name, TsEntityName::Ident(name) if name.sym == "const")),
        _ => false,
    }
}
