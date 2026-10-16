use std::path::Path;

use rnix::ast;
use snafu::OptionExt;

use crate::error::{NixParseSnafu, Result};

pub fn parse_ast(src: &str, logical_path: &Path) -> Result<rnix::Parse<rnix::Root>> {
    let parse = rnix::Root::parse(src);
    let errors = parse.errors();
    if !errors.is_empty() {
        return NixParseSnafu {
            path: logical_path.to_path_buf(),
            reason: format!("syntax errors: {:?}", errors),
        }
        .fail();
    }
    Ok(parse)
}

pub fn unwrap_attr_set(expr: ast::Expr) -> Option<ast::AttrSet> {
    match expr {
        ast::Expr::AttrSet(s) => Some(s),
        ast::Expr::Lambda(l) => l.body().and_then(unwrap_attr_set),
        ast::Expr::LetIn(let_in) => let_in.body().and_then(unwrap_attr_set),
        _ => None,
    }
}

pub fn attr_to_name(attr: &ast::Attr) -> Option<String> {
    match attr {
        ast::Attr::Ident(ident) => Some(ident.to_string()),
        ast::Attr::Str(s) => Some(s.to_string().trim_matches('"').to_string()),
        _ => None,
    }
}

pub fn parse_root_attr_set(src: &str, path: &Path) -> Result<ast::AttrSet> {
    let parse = parse_ast(src, path)?;
    let root_expr = parse.tree().expr().context(NixParseSnafu {
        path: path.to_path_buf(),
        reason: "empty Nix expression",
    })?;
    unwrap_attr_set(root_expr).context(NixParseSnafu {
        path: path.to_path_buf(),
        reason: "expected attribute set at root",
    })
}
