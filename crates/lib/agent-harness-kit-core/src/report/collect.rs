//! `Report`: the findings of a run, always ordered, and its text and JSON
//! forms.
// @zen-component: KIT-Report

use serde::{
    Serialize,
    ser::{SerializeMap, SerializeSeq, SerializeStruct},
};

use crate::report::{Finding, Severity};

/// Errors, warnings and info, each list ordered by path, line and message
/// without duplicates.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    errors: Vec<Finding>,
    warnings: Vec<Finding>,
    info: Vec<Finding>,
}

/// The order of a list: path, then line, then message.
fn key(f: &Finding) -> (&str, Option<usize>, &str) {
    (&f.path, f.line, &f.message)
}

impl Report {
    /// Add a finding to the list its severity selects, at its place in the
    /// order; an exact duplicate is dropped.
    // @zen-impl: KIT-13_AC-2
    pub fn push(&mut self, finding: Finding) {
        let list = match finding.severity {
            Severity::Error => &mut self.errors,
            Severity::Warning => &mut self.warnings,
            Severity::Info => &mut self.info,
        };
        let k = key(&finding);
        let start = list.partition_point(|f| key(f) < k);
        let end = start + list[start..].partition_point(|f| key(f) == k);
        if !list[start..end].contains(&finding) {
            list.insert(end, finding);
        }
    }

    /// The errors.
    pub fn errors(&self) -> &[Finding] {
        &self.errors
    }

    /// The warnings.
    pub fn warnings(&self) -> &[Finding] {
        &self.warnings
    }

    /// The info findings.
    pub fn info(&self) -> &[Finding] {
        &self.info
    }

    /// Whether the run found an error.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// The text report: the errors, plus the warnings and info findings when
    /// `show` names their severity, ordered together by path then line, one
    /// `<path>:<line>: <level>: <message> (<authority>)` line each; then the
    /// counts line, e.g. `1 error, 0 warnings, 2 info`.
    // @zen-impl: KIT-13_AC-3
    pub fn to_text(&self, show: &[Severity]) -> String {
        let mut findings: Vec<&Finding> = self.errors.iter().collect();
        if show.contains(&Severity::Warning) {
            findings.extend(&self.warnings);
        }
        if show.contains(&Severity::Info) {
            findings.extend(&self.info);
        }
        findings.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));
        let mut out = String::new();
        for f in findings {
            out.push_str(&f.to_line());
            out.push('\n');
        }
        out.push_str(&format!(
            "{}, {}, {} info\n",
            plural(self.errors.len(), "error"),
            plural(self.warnings.len(), "warning"),
            self.info.len()
        ));
        out
    }
}

fn plural(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

impl Extend<Finding> for Report {
    fn extend<I: IntoIterator<Item = Finding>>(&mut self, findings: I) {
        for f in findings {
            self.push(f);
        }
    }
}

impl FromIterator<Finding> for Report {
    fn from_iter<I: IntoIterator<Item = Finding>>(findings: I) -> Self {
        let mut r = Report::default();
        r.extend(findings);
        r
    }
}

/// One finding in the JSON form: `path`, `line` when there is one,
/// `message`, and `authority` when there is one.
struct Entry<'a>(&'a Finding);

impl Serialize for Entry<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(None)?;
        m.serialize_entry("path", &self.0.path)?;
        if let Some(line) = self.0.line {
            m.serialize_entry("line", &line)?;
        }
        m.serialize_entry("message", &self.0.message)?;
        if let Some(authority) = &self.0.authority {
            m.serialize_entry("authority", authority)?;
        }
        m.end()
    }
}

struct Entries<'a>(&'a [Finding]);

impl Serialize for Entries<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut seq = s.serialize_seq(Some(self.0.len()))?;
        for f in self.0 {
            seq.serialize_element(&Entry(f))?;
        }
        seq.end()
    }
}

/// `{"errors": [...], "warnings": [...], "info": [...]}`.
// @zen-impl: KIT-13_AC-4
impl Serialize for Report {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut st = s.serialize_struct("Report", 3)?;
        st.serialize_field("errors", &Entries(&self.errors))?;
        st.serialize_field("warnings", &Entries(&self.warnings))?;
        st.serialize_field("info", &Entries(&self.info))?;
        st.end()
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn report() -> Report {
        [
            Finding::error("b.yaml", "bad").line(2).authority("CFG-1"),
            Finding::warning("a.yaml", "odd").authority("CFG-2"),
            Finding::info("a.yaml", "note").line(1),
        ]
        .into_iter()
        .collect()
    }

    // @zen-test: KIT-13_AC-4
    #[test]
    fn json_has_three_lists() {
        let r: Report = [
            Finding::error("a.yaml", "x").line(41).authority("CFG-1"),
            Finding::warning("b.yaml", "w"),
            Finding::info("c.toml", "i").authority("CLI-3"),
        ]
        .into_iter()
        .collect();
        assert_eq!(
            serde_json::to_value(&r).unwrap(),
            serde_json::json!({
                "errors": [{"path": "a.yaml", "line": 41, "message": "x", "authority": "CFG-1"}],
                "warnings": [{"path": "b.yaml", "message": "w"}],
                "info": [{"path": "c.toml", "message": "i", "authority": "CLI-3"}]
            })
        );
        assert!(r.has_errors());
        assert!(!Report::default().has_errors());
    }

    // @zen-test: KIT-13_AC-3
    #[test]
    fn errors_only_by_default() {
        insta::assert_snapshot!(report().to_text(&[]), @r"
        b.yaml:2: error: bad (CFG-1)
        1 error, 1 warning, 1 info
        ");
    }

    // @zen-test: KIT-13_AC-3
    #[test]
    fn warnings_and_info_are_listed_in_path_order_when_asked() {
        insta::assert_snapshot!(report().to_text(&[Severity::Warning, Severity::Info]), @r"
        a.yaml: warning: odd (CFG-2)
        a.yaml:1: info: note
        b.yaml:2: error: bad (CFG-1)
        1 error, 1 warning, 1 info
        ");
        assert_eq!(
            Report::default().to_text(&[Severity::Warning, Severity::Info]),
            "0 errors, 0 warnings, 0 info\n"
        );
        assert_eq!(
            report().to_text(&[Severity::Info]),
            "a.yaml:1: info: note\nb.yaml:2: error: bad (CFG-1)\n1 error, 1 warning, 1 info\n"
        );
    }

    fn arb_finding() -> impl Strategy<Value = Finding> {
        (
            prop::sample::select(vec!["a.md", "b/c.md", "b/a.md", "z.md"]),
            prop::option::of(1usize..50),
            prop::sample::select(vec!["m1", "m2"]),
            prop::sample::select(vec![Severity::Error, Severity::Warning, Severity::Info]),
            prop::option::of(prop::sample::select(vec!["A-1", "A-2"])),
        )
            .prop_map(|(path, line, msg, severity, authority)| {
                let mut f = Finding::new(severity, path, msg);
                f.line = line;
                f.authority = authority.map(str::to_string);
                f
            })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        // @zen-test: KIT_P-8
        // @zen-test: KIT-13_AC-2
        #[test]
        fn pushes_keep_each_list_ordered_without_duplicates(findings in prop::collection::vec(arb_finding(), 0..12)) {
            let r: Report = findings.iter().cloned().collect();
            for list in [r.errors(), r.warnings(), r.info()] {
                for w in list.windows(2) {
                    prop_assert!(key(&w[0]) <= key(&w[1]));
                }
                for (i, a) in list.iter().enumerate() {
                    prop_assert!(!list[i + 1..].contains(a));
                }
            }
            prop_assert!(r.errors().iter().all(|f| f.severity == Severity::Error));
            prop_assert!(r.warnings().iter().all(|f| f.severity == Severity::Warning));
            prop_assert!(r.info().iter().all(|f| f.severity == Severity::Info));
            let mut expected = findings.clone();
            expected.sort();
            expected.dedup();
            prop_assert_eq!(r.errors().len() + r.warnings().len() + r.info().len(), expected.len());
        }

        // @zen-test: KIT-13_AC-4
        #[test]
        fn json_shape(findings in prop::collection::vec(arb_finding(), 0..6)) {
            let r: Report = findings.into_iter().collect();
            let text = serde_json::to_string(&r).unwrap();
            let e = text.find("\"errors\":").unwrap();
            let w = text.find("\"warnings\":").unwrap();
            let i = text.find("\"info\":").unwrap();
            prop_assert!(e < w && w < i, "{text}");
            let json: serde_json::Value = serde_json::from_str(&text).unwrap();
            let obj = json.as_object().unwrap();
            prop_assert_eq!(obj.len(), 3);
            for list in obj.values() {
                for entry in list.as_array().unwrap() {
                    let e = entry.as_object().unwrap();
                    prop_assert!(e.contains_key("path") && e.contains_key("message"));
                    prop_assert!(e.keys().all(|k| ["path", "line", "message", "authority"].contains(&k.as_str())));
                }
            }
        }
    }
}
