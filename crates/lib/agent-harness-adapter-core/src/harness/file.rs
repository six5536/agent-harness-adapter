//! The `file` kind: the tool owns the whole file.

use crate::{
    harness::part::{Kind, Part},
    hash::normalise,
};

/// The files a `file` part writes, each with LF line endings; empty for a
/// part of another kind.
pub(crate) fn render_files(part: &Part) -> Vec<(String, String)> {
    match &part.kind {
        Kind::Files { files, .. } => files
            .iter()
            .map(|(p, t)| (p.clone(), normalise(t)))
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // @zen-test: KIT-2_AC-1
    #[test]
    fn renders_each_file_with_lf() {
        let part = Part::files("x", "d", vec![("a.md".into(), "a\r\nb\r\n".into())]);
        assert_eq!(
            render_files(&part),
            vec![("a.md".to_string(), "a\nb\n".to_string())]
        );
        assert!(render_files(&Part::region("i", "N.md", "b")).is_empty());
    }
}
