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

const MAX_UNWRAP_DEPTH: usize = 32;

pub fn unwrap_attr_set(mut expr: ast::Expr) -> Option<ast::AttrSet> {
    for _ in 0..MAX_UNWRAP_DEPTH {
        match expr {
            ast::Expr::AttrSet(s) => return Some(s),
            ast::Expr::Paren(p) => expr = p.expr()?,
            ast::Expr::Lambda(l) => expr = l.body()?,
            ast::Expr::LetIn(let_in) => expr = let_in.body()?,
            _ => return None,
        }
    }
    None
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unwrap_attr_set_parenthesized() {
        let src = "({ a = 1; })";
        let parse = rnix::Root::parse(src);
        let root = parse.tree().expr().unwrap();
        assert!(unwrap_attr_set(root).is_some());
    }
}
