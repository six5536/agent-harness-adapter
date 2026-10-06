//! The `region` kind: the tool owns one block between its markers in the
//! user's file.
// @zen-component: KIT-Region

use crate::hash::normalise;

/// A tool's region markers: `<!-- <tool>:harness -->` and
/// `<!-- /<tool>:harness -->`, each on a line of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Markers {
    open: String,
    close: String,
}

impl Markers {
    /// The markers of the tool named `tool`.
    pub fn new(tool: &str) -> Self {
        Markers {
            open: format!("<!-- {tool}:harness -->"),
            close: format!("<!-- /{tool}:harness -->"),
        }
    }

    /// The opening marker.
    pub fn open(&self) -> &str {
        &self.open
    }

    /// The closing marker.
    pub fn close(&self) -> &str {
        &self.close
    }

    /// The line indexes of the markers in LF text: the first closing line
    /// that has an opening line before it, and the nearest such opening
    /// line. A stray opening marker (its closing one deleted) is then just
    /// the user's text: the region never swallows what lies between it and
    /// a later region, so `--force` cannot delete it.
    // @zen-impl: KIT-9_AC-1
    fn locate(&self, lines: &[&str]) -> Option<(usize, usize)> {
        lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.trim_end() == self.close)
            .find_map(|(close, _)| {
                let open = lines[..close]
                    .iter()
                    .rposition(|l| l.trim_end() == self.open)?;
                Some((open, close))
            })
    }

    /// The block with its markers, LF, a blank line after the opening
    /// marker.
    fn framed(&self, block: &str) -> String {
        let mut body = strip_leading_blank_lines(&normalise(block));
        if !body.is_empty() && !body.ends_with('\n') {
            body.push('\n');
        }
        format!("{}\n\n{body}{}\n", self.open, self.close)
    }

    /// The block between the markers in `text`, LF, without the blank lines
    /// that open it and with its trailing newline; `None` when the markers
    /// are absent.
    pub fn find(&self, text: &str) -> Option<String> {
        let text = normalise(text);
        let lines: Vec<&str> = text.split('\n').collect();
        let (open, close) = self.locate(&lines)?;
        let inner = &lines[open + 1..close];
        if inner.is_empty() {
            return Some(String::new());
        }
        Some(strip_leading_blank_lines(&format!(
            "{}\n",
            inner.join("\n")
        )))
    }

    /// The file to write: `block` rewritten between the markers, appended
    /// after one blank line when they are absent, or alone for an absent or
    /// empty file. The file's line endings are kept, and nothing outside the
    /// region changes. `None` when nothing changes.
    // @zen-impl: KIT-9_AC-2
    // @zen-impl: KIT-9_AC-3
    pub fn render(&self, existing: Option<&str>, block: &str) -> Option<String> {
        let new_block = self.framed(block);
        let Some(existing) = existing else {
            return Some(new_block);
        };
        let crlf = existing.contains("\r\n");
        let text = normalise(existing);
        let lines: Vec<&str> = text.split('\n').collect();
        let out = match self.locate(&lines) {
            Some((open, close)) => {
                let head = lines[..open].join("\n");
                let tail = lines[close + 1..].join("\n");
                let head = if open == 0 { head } else { format!("{head}\n") };
                format!("{head}{new_block}{tail}")
            }
            None if text.is_empty() => new_block,
            None => {
                let mut head = text.clone();
                if !head.ends_with('\n') {
                    head.push('\n');
                }
                format!("{head}\n{new_block}")
            }
        };
        let out = if crlf { out.replace('\n', "\r\n") } else { out };
        (out != existing).then_some(out)
    }
}

/// LF text without its leading whitespace-only lines.
fn strip_leading_blank_lines(text: &str) -> String {
    let mut rest = text;
    while let Some(i) = rest.find('\n') {
        if !rest[..i].trim().is_empty() {
            break;
        }
        rest = &rest[i + 1..];
    }
    if rest.trim().is_empty() {
        String::new()
    } else {
        rest.to_string()
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    const BLOCK: &str = "Read the skill.\n";

    fn m() -> Markers {
        Markers::new("tool")
    }

    #[test]
    fn markers_carry_the_tool_name() {
        assert_eq!(m().open(), "<!-- tool:harness -->");
        assert_eq!(m().close(), "<!-- /tool:harness -->");
    }

    // @zen-test: KIT-9_AC-2
    #[test]
    fn a_new_file_is_the_block_alone() {
        let out = m().render(None, BLOCK).unwrap();
        assert_eq!(
            out,
            "<!-- tool:harness -->\n\nRead the skill.\n<!-- /tool:harness -->\n"
        );
        assert_eq!(m().find(&out).as_deref(), Some(BLOCK));
        assert_eq!(m().render(Some(&out), BLOCK), None);
        // A block with no trailing newline gets one; an empty block is
        // the markers alone.
        assert_eq!(
            m().render(None, "x").unwrap(),
            "<!-- tool:harness -->\n\nx\n<!-- /tool:harness -->\n"
        );
        assert_eq!(
            m().render(None, "").unwrap(),
            "<!-- tool:harness -->\n\n<!-- /tool:harness -->\n"
        );
    }

    // @zen-test: KIT-9_AC-2
    #[test]
    fn markers_absent_appends_after_one_blank_line() {
        let out = m().render(Some("# Title\n\nText."), BLOCK).unwrap();
        assert_eq!(
            out,
            "# Title\n\nText.\n\n<!-- tool:harness -->\n\nRead the skill.\n<!-- /tool:harness -->\n"
        );
        assert_eq!(m().render(Some(""), BLOCK).unwrap(), m().framed(BLOCK));
        assert_eq!(
            m().render(Some("x\n"), BLOCK).unwrap(),
            format!("x\n\n{}", m().framed(BLOCK))
        );
    }

    // @zen-test: KIT-9_AC-2
    // @zen-test: KIT-9_AC-3
    #[test]
    fn markers_present_rewrites_between_the_first_ones_only() {
        let before =
            "# Title\n\n<!-- tool:harness -->\nold\n<!-- /tool:harness -->\n\n## After\n\nkept\n";
        let out = m().render(Some(before), BLOCK).unwrap();
        assert_eq!(
            out,
            "# Title\n\n<!-- tool:harness -->\n\nRead the skill.\n<!-- /tool:harness -->\n\n## After\n\nkept\n"
        );
        assert_eq!(m().render(Some(&out), BLOCK), None);
        let before = "<!-- tool:harness -->\n<!-- /tool:harness -->\ntail\n";
        assert_eq!(m().find(before).as_deref(), Some(""));
        assert_eq!(
            m().render(Some(before), BLOCK).unwrap(),
            "<!-- tool:harness -->\n\nRead the skill.\n<!-- /tool:harness -->\ntail\n"
        );
        // Another tool's markers are text like any other.
        let other = "<!-- other:harness -->\nx\n<!-- /other:harness -->\n";
        assert_eq!(m().find(other), None);
        assert!(m().render(Some(other), BLOCK).unwrap().starts_with(other));
    }

    // A stray opening marker is the user's text: the region is the one a
    // closing marker ends, so it never spans the user's lines.
    // @zen-test: KIT-9_AC-1
    #[test]
    fn a_stray_opening_marker_is_left_alone() {
        let stray = "<!-- tool:harness -->\nmine\n";
        assert_eq!(m().find(stray), None);
        let out = m().render(Some(stray), BLOCK).unwrap();
        assert_eq!(out, format!("{stray}\n{}", m().framed(BLOCK)));
        assert_eq!(m().find(&out).as_deref(), Some(BLOCK));
        assert_eq!(m().render(Some(&out), BLOCK), None, "current");
        let again = m().render(Some(&out), "New.\n").unwrap();
        assert!(again.starts_with(stray), "{again}");
    }

    // @zen-test: KIT-9_AC-2
    #[test]
    fn keeps_crlf_line_endings() {
        let before = "a\r\n\r\n<!-- tool:harness -->\r\nold\r\n<!-- /tool:harness -->\r\n";
        let out = m().render(Some(before), BLOCK).unwrap();
        assert_eq!(
            out,
            "a\r\n\r\n<!-- tool:harness -->\r\n\r\nRead the skill.\r\n<!-- /tool:harness -->\r\n"
        );
        assert_eq!(m().find(&out).as_deref(), Some(BLOCK));
        assert_eq!(m().render(Some(&out), BLOCK), None);
        let out = m().render(Some("a\r\n"), BLOCK).unwrap();
        assert!(out.contains("\r\n<!-- tool:harness -->\r\n"), "{out:?}");
    }

    // @zen-test: KIT-9_AC-1
    #[test]
    fn a_closing_marker_before_the_opening_one_is_no_block() {
        assert_eq!(
            m().find("<!-- /tool:harness -->\n<!-- tool:harness -->\n"),
            None
        );
        assert_eq!(m().find("no markers"), None);
    }

    fn arb_text() -> impl Strategy<Value = String> {
        prop::collection::vec("[a-z #@-]{0,12}", 0..6).prop_map(|lines| lines.join("\n"))
    }

    fn arb_block() -> impl Strategy<Value = String> {
        prop::collection::vec("[a-zA-Z.][a-zA-Z .]{0,19}", 1..4)
            .prop_map(|lines| format!("{}\n", lines.join("\n")))
    }

    /// The markers' lines removed with the block between them.
    fn strip_block(t: &str) -> String {
        let lines: Vec<&str> = t.split('\n').collect();
        match (
            lines.iter().position(|l| *l == "<!-- tool:harness -->"),
            lines.iter().position(|l| *l == "<!-- /tool:harness -->"),
        ) {
            (Some(o), Some(c)) if c > o => [&lines[..o], &lines[c + 1..]].concat().join("\n"),
            _ => t.to_string(),
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        // @zen-test: KIT_P-1
        #[test]
        fn region_touches_only_its_block(text in arb_text(), block in arb_block(), crlf in any::<bool>()) {
            let text = if crlf { text.replace('\n', "\r\n") } else { text };
            let Some(after) = m().render(Some(&text), &block) else {
                let found = m().find(&text);
                prop_assert_eq!(found.as_deref(), Some(block.as_str()));
                return Ok(());
            };
            let lf = after.replace("\r\n", "\n");
            prop_assert_eq!(lf.matches("<!-- tool:harness -->").count(), 1);
            prop_assert!(lf.contains("<!-- tool:harness -->\n\n"), "a blank line after the opening marker");
            let found = m().find(&after);
            prop_assert_eq!(found.as_deref(), Some(block.as_str()));
            let kept = strip_block(&lf);
            let original = strip_block(&text.replace("\r\n", "\n"));
            prop_assert!(kept.starts_with(original.trim_end_matches('\n')), "{kept:?} vs {original:?}");
            prop_assert_eq!(m().render(Some(&after), &block), None);
            prop_assert_eq!(after.contains("\r\n"), text.contains("\r\n"));
        }
    }
}
