//! The declined parts: a store behind a trait, and a TOML implementation
//! that keeps `[harness.<name>] without = [...]` in the tool's config file,
//! which the user writes and the library only reads.
// @zen-component: KIT-Declined

use std::path::PathBuf;

use toml_edit::{DocumentMut, Item, TableLike};

use crate::{Error, Result, fs::read_text, harness::parse_toml};

/// Where the parts a user declined are kept.
pub trait DeclinedStore {
    /// The declined parts of `harness`. A store that cannot be read is a
    /// refusal; `install` calls this before any write.
    fn declined(&self, harness: &str) -> Result<Vec<String>>;
    /// Where the user declines parts, e.g. `.tool/config.toml`, named when
    /// a forced part is declined.
    fn location(&self) -> String {
        "the tool's config".to_string()
    }
}

/// A [`DeclinedStore`] in a TOML file: `[harness.<name>] without = [...]`.
// @zen-impl: KIT-8_AC-3
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TomlDeclined {
    path: PathBuf,
    display: String,
}

impl TomlDeclined {
    /// The store in the file `path` (e.g. `.tool/config.toml`), named
    /// `display` in refusals.
    pub fn new(path: impl Into<PathBuf>, display: impl Into<String>) -> Self {
        TomlDeclined {
            path: path.into(),
            display: display.into(),
        }
    }

    fn refuse(&self, m: &str) -> Error {
        Error::file(&self.display, m)
    }

    fn parse(&self, text: &str) -> Result<DocumentMut> {
        parse_toml(&self.display, text)
    }
}

/// `Some(None)` for no item, `Some(Some(table))` for a table, `None` for
/// an item of another type.
fn table(item: Option<&Item>) -> Option<Option<&dyn TableLike>> {
    match item {
        None => Some(None),
        Some(i) => i.as_table_like().map(Some),
    }
}

impl DeclinedStore for TomlDeclined {
    fn declined(&self, harness: &str) -> Result<Vec<String>> {
        let Some(text) = read_text(&self.path)? else {
            return Ok(Vec::new());
        };
        let doc = self.parse(&text)?;
        let Some(h) =
            table(doc.get("harness")).ok_or_else(|| self.refuse("`harness` is not a table"))?
        else {
            return Ok(Vec::new());
        };
        let Some(t) = table(h.get(harness))
            .ok_or_else(|| self.refuse(&format!("`harness.{harness}` is not a table")))?
        else {
            return Ok(Vec::new());
        };
        let Some(without) = t.get("without") else {
            return Ok(Vec::new());
        };
        let not_list = || {
            self.refuse(&format!(
                "`harness.{harness}.without` is not a list of part names"
            ))
        };
        without
            .as_array()
            .ok_or_else(not_list)?
            .iter()
            .map(|v| v.as_str().map(str::to_string).ok_or_else(not_list))
            .collect()
    }

    fn location(&self) -> String {
        self.display.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::temp_dir;

    // @zen-test: KIT-8_AC-3
    #[test]
    fn the_toml_store_reads() {
        let dir = temp_dir("declined-store");
        let store = TomlDeclined::new(dir.join("sub/config.toml"), "sub/config.toml");
        assert_eq!(store.location(), "sub/config.toml");
        assert_eq!(store.declined("claude").unwrap(), Vec::<String>::new());
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        let text = "# mine\n[harness.claude] # kept\nwithout = [\"hooks\"]\n";
        std::fs::write(&store.path, text).unwrap();
        assert_eq!(store.declined("claude").unwrap(), vec!["hooks".to_string()]);
        assert_eq!(store.declined("other").unwrap(), Vec::<String>::new());
        assert_eq!(std::fs::read_to_string(&store.path).unwrap(), text);
        std::fs::write(
            &store.path,
            "harness = { claude = { without = [\"skills\"] } }\n",
        )
        .unwrap();
        assert_eq!(
            store.declined("claude").unwrap(),
            vec!["skills".to_string()]
        );
        for (text, empty) in [("[machines]\n", true), ("[harness.claude]\nx = 1\n", true)] {
            std::fs::write(&store.path, text).unwrap();
            assert_eq!(store.declined("claude").unwrap().is_empty(), empty);
        }
        for (text, why) in [
            ("harness = 1\n", "`harness` is not a table"),
            ("[harness]\nclaude = 1\n", "`harness.claude` is not a table"),
            (
                "[harness.claude]\nwithout = 1\n",
                "is not a list of part names",
            ),
            (
                "[harness.claude]\nwithout = [1]\n",
                "is not a list of part names",
            ),
            ("[harness\n", "sub/config.toml: "),
        ] {
            std::fs::write(&store.path, text).unwrap();
            let e = store.declined("claude").unwrap_err();
            assert!(e.to_string().contains(why), "{e}");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
