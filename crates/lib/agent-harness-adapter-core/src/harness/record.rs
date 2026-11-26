//! The record: for each installed harness, the fingerprint of each part
//! the tool wrote, in `same-same`'s text form. A TOML file whose path and
//! header line the tool supplies.

use std::{collections::BTreeMap, fmt::Write as _, path::Path};

use crate::{Error, Result, fs::read_text, harness::parse_toml};

/// The record: harness name to part name to the part's recorded value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Record {
    /// One table per installed harness.
    pub(crate) harnesses: BTreeMap<String, BTreeMap<String, Recorded>>,
}

/// A part's recorded value: one fingerprint for a region, merge, or
/// external part, and one for each file of a file part, by its path in the
/// part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Recorded {
    /// A region's, a merge's, or an external part's.
    One(String),
    /// A file part's, by each file's path in the part.
    Files(BTreeMap<String, String>),
}

/// Read the record at `path`; empty when absent, a refusal naming
/// `display` when it does not parse or holds for a part anything but a
/// string or a table of strings.
// sokf-implements: KIT-7_AC-2
pub(crate) fn read_record(path: &Path, display: &str) -> Result<Record> {
    let Some(text) = read_text(path)? else {
        return Ok(Record::default());
    };
    let refuse = |m: &str| Error::file(display, m);
    let doc = parse_toml(display, &text)?;
    let mut harnesses = BTreeMap::new();
    for (name, item) in doc.iter() {
        let table = item
            .as_table_like()
            .ok_or_else(|| refuse(&format!("`{name}` is not a table")))?;
        let mut parts = BTreeMap::new();
        for (part, value) in table.iter() {
            let recorded = if let Some(value) = value.as_str() {
                Recorded::One(value.to_string())
            } else if let Some(files) = value.as_table_like() {
                let mut out = BTreeMap::new();
                for (file, value) in files.iter() {
                    let value = value.as_str().ok_or_else(|| {
                        refuse(&format!("`{name}.{part}.{file}` is not a string"))
                    })?;
                    out.insert(file.to_string(), value.to_string());
                }
                Recorded::Files(out)
            } else {
                return Err(refuse(&format!(
                    "`{name}.{part}` is neither a string nor a table"
                )));
            };
            parts.insert(part.to_string(), recorded);
        }
        harnesses.insert(name.to_string(), parts);
    }
    Ok(Record { harnesses })
}

/// A TOML key: bare when it can be, else a basic string.
fn key(key: &str) -> String {
    let bare = !key.is_empty()
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    if bare {
        key.to_string()
    } else {
        toml_edit::Value::from(key).to_string()
    }
}

/// The record file's text: `header`, then one table per harness in name
/// order, with the values of its region, merge, and external parts in name
/// order, then a table under it for each file part, with its files' values
/// in path order; LF. A harness with no value (every part shared or
/// declined) keeps an empty table: it is installed, and stays in the set.
// sokf-implements: KIT-19_AC-1
// sokf-implements: KIT-7_AC-2
pub(crate) fn render_record(record: &Record, header: &str) -> String {
    let mut out = format!("{header}\n");
    for (name, parts) in &record.harnesses {
        let _ = writeln!(out, "\n[{}]", key(name));
        for (part, recorded) in parts {
            if let Recorded::One(value) = recorded {
                let _ = writeln!(
                    out,
                    "{} = {}",
                    key(part),
                    toml_edit::Value::from(value.as_str())
                );
            }
        }
        for (part, recorded) in parts {
            if let Recorded::Files(files) = recorded {
                let _ = writeln!(out, "\n[{}.{}]", key(name), key(part));
                for (file, value) in files {
                    let _ = writeln!(
                        out,
                        "{} = {}",
                        key(file),
                        toml_edit::Value::from(value.as_str())
                    );
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::test_support::temp_dir;

    const HEADER: &str = "# Written by tool harness install. Do not edit.";

    fn one(value: &str) -> Recorded {
        Recorded::One(value.to_string())
    }

    // sokf-tests: KIT-7_AC-2
    #[test]
    fn reads_and_renders_the_record() {
        let dir = temp_dir("record-rw");
        let path = dir.join("harness.toml");
        assert_eq!(
            read_record(&path, "harness.toml").unwrap(),
            Record::default()
        );
        let mut record = Record::default();
        record.harnesses.insert(
            "claude".into(),
            BTreeMap::from([
                (
                    "skills".to_string(),
                    Recorded::Files(BTreeMap::from([
                        ("tool/SKILL.md".to_string(), "ss1:md.1:01".to_string()),
                        ("tool/a b.sh".to_string(), "ss1:bytes:02".to_string()),
                    ])),
                ),
                ("hooks".to_string(), one("ss1:bytes:03")),
                ("instructions".to_string(), one("ss1:md.1:04")),
            ]),
        );
        record.harnesses.insert(
            "aider".into(),
            BTreeMap::from([("x".to_string(), one("ss1:bytes:05"))]),
        );
        let text = render_record(&record, HEADER);
        assert_eq!(
            text,
            "# Written by tool harness install. Do not edit.\n\n[aider]\nx = \"ss1:bytes:05\"\n\n[claude]\nhooks = \"ss1:bytes:03\"\ninstructions = \"ss1:md.1:04\"\n\n[claude.skills]\n\"tool/SKILL.md\" = \"ss1:md.1:01\"\n\"tool/a b.sh\" = \"ss1:bytes:02\"\n"
        );
        fs::write(&path, &text).unwrap();
        assert_eq!(read_record(&path, "harness.toml").unwrap(), record);
        // An empty table stays, and so does a file part with no file.
        record.harnesses.insert("empty".into(), BTreeMap::new());
        record
            .harnesses
            .get_mut("aider")
            .unwrap()
            .insert("skills".into(), Recorded::Files(BTreeMap::new()));
        let with_empty = render_record(&record, HEADER);
        assert!(with_empty.contains("\n[empty]\n"), "{with_empty}");
        assert!(with_empty.contains("\n[aider.skills]\n"), "{with_empty}");
        fs::write(&path, &with_empty).unwrap();
        assert_eq!(read_record(&path, "harness.toml").unwrap(), record);
        // A record of 0.4.0 reads, its values kept as they are.
        fs::write(&path, "[claude]\nskills = \"fnv1a64:0123456789abcdef\"\n").unwrap();
        assert_eq!(
            read_record(&path, "harness.toml").unwrap().harnesses["claude"]["skills"],
            one("fnv1a64:0123456789abcdef")
        );
        for (bad, why) in [
            (
                "[claude]\nskills = 1\n",
                "`claude.skills` is neither a string nor a table",
            ),
            (
                "[claude.skills]\n\"a.md\" = 1\n",
                "`claude.skills.a.md` is not a string",
            ),
            ("claude = 1\n", "`claude` is not a table"),
            ("[claude\n", ""),
        ] {
            fs::write(&path, bad).unwrap();
            let e = read_record(&path, "harness.toml").unwrap_err();
            assert!(matches!(e, Error::File { .. }), "{e}");
            assert!(e.to_string().starts_with("harness.toml: "), "{e}");
            assert!(e.to_string().contains(why), "{e}");
        }
        fs::remove_dir_all(&dir).unwrap();
    }
}
