//! Small TOML documents a harness reads as whole files (Codex agents,
//! Gemini CLI commands), written with `toml_edit` so strings are escaped.

use toml_edit::{DocumentMut, value};

/// A TOML document of string keys, in order.
pub(crate) fn strings(pairs: &[(&str, &str)]) -> String {
    let mut doc = DocumentMut::new();
    for (k, v) in pairs {
        doc[*k] = value(*v);
    }
    doc.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_are_escaped() {
        let text = strings(&[("name", "r"), ("prompt", "a \"b\"\nc\n")]);
        let doc: DocumentMut = text.parse().unwrap();
        assert_eq!(doc["name"].as_str(), Some("r"));
        assert_eq!(doc["prompt"].as_str(), Some("a \"b\"\nc\n"));
        assert!(text.starts_with("name = \"r\"\n"), "{text}");
    }
}
