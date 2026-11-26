//! The JSON merge's writer of text spans: a file's text made to hold a new
//! value with only the text of what changed rewritten, and every other byte
//! kept. jsonc-parser's syntax tree gives the span of each member and item.

use jsonc_parser::{
    CollectOptions, ParseOptions,
    ast::{self, ObjectProp},
    common::Ranged,
    parse_to_ast,
};
use serde_json::{Map, Value};

/// `text`, whose value is `old`, rewritten to hold `new`: each member or
/// item that changed is rewritten, removed, or added beside its siblings in
/// their layout; `None` when the syntax tree doesn't match `old`, such as a
/// file with a duplicate key.
// sokf-implements: KIT-10_AC-1
pub(super) fn patch(text: &str, old: &Value, new: &Value, unit: &str) -> Option<String> {
    let ast = parse_to_ast(text, &CollectOptions::default(), &ParseOptions::default())
        .ok()?
        .value?;
    if !matches(&ast, old) {
        return None;
    }
    let mut splices = Splices {
        text,
        unit,
        eol: if text.contains("\r\n") { "\r\n" } else { "\n" },
        edits: Vec::new(),
    };
    splices.value(&ast, old, new)?;
    let mut edits = splices.edits;
    // From the end, so that each span is still where the tree found it; at
    // one place a removal goes before an insertion.
    edits.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    let mut out = text.to_string();
    for (start, end, with) in edits {
        out.replace_range(start..end, &with);
    }
    Some(out)
}

/// The edits of one text: spans of bytes and their new text.
struct Splices<'t> {
    text: &'t str,
    /// The file's indent, for a container that has no member to copy.
    unit: &'t str,
    /// The file's line ending, for the lines that an edit adds.
    eol: &'static str,
    edits: Vec<(usize, usize, String)>,
}

/// One member of an object or item of an array, by its span.
struct Slot {
    start: usize,
    end: usize,
}

/// What a container holds after the edit, in order: a slot of the text,
/// kept, or a new member or item, rendered.
enum Entry {
    Kept(usize),
    New(Value, Option<String>),
}

impl Splices<'_> {
    /// Edit the node `ast`, whose value is `old`, to hold `new`.
    fn value(&mut self, ast: &ast::Value<'_>, old: &Value, new: &Value) -> Option<()> {
        if old == new {
            return Some(());
        }
        match (ast, old, new) {
            (ast::Value::Object(obj), Value::Object(o), Value::Object(n)) => self.object(obj, o, n),
            (ast::Value::Array(arr), Value::Array(o), Value::Array(n)) => self.array(arr, o, n),
            _ => {
                let indent = line_indent(self.text, ast.start()).to_string();
                self.edits.push((
                    ast.start(),
                    ast.end(),
                    pretty(new, &indent, self.unit, self.eol),
                ));
                Some(())
            }
        }
    }

    fn object(
        &mut self,
        obj: &ast::Object<'_>,
        old: &Map<String, Value>,
        new: &Map<String, Value>,
    ) -> Option<()> {
        let props: &[ObjectProp<'_>] = &obj.properties;
        if props.len() != old.len()
            || props
                .iter()
                .zip(old.keys())
                .any(|(p, k)| p.name.as_str() != k)
        {
            return None;
        }
        for (prop, (key, value)) in props.iter().zip(old) {
            if let Some(wanted) = new.get(key) {
                self.value(&prop.value, value, wanted)?;
            }
        }
        // Each new member goes after the kept one before it in the new order.
        let entries: Vec<Entry> = new
            .iter()
            .map(|(key, value)| match old.keys().position(|k| k == key) {
                Some(i) => Entry::Kept(i),
                None => Entry::New(value.clone(), Some(key.clone())),
            })
            .collect();
        let slots: Vec<Slot> = props
            .iter()
            .map(|p| Slot {
                start: p.start(),
                end: p.end(),
            })
            .collect();
        self.container(obj.start(), obj.end(), &slots, &entries);
        Some(())
    }

    fn array(&mut self, arr: &ast::Array<'_>, old: &[Value], new: &[Value]) -> Option<()> {
        if arr.elements.len() != old.len() {
            return None;
        }
        let mut entries = Vec::new();
        let (mut i, mut j) = (0, 0);
        for (oi, nj) in common(old, new).into_iter().chain([(old.len(), new.len())]) {
            // The gap before a common item: items of one kind of container
            // pair up and are edited in place, the rest go or come.
            while i < oi && j < nj && same_kind(&old[i], &new[j]) {
                self.value(&arr.elements[i], &old[i], &new[j])?;
                entries.push(Entry::Kept(i));
                i += 1;
                j += 1;
            }
            i = oi;
            while j < nj {
                entries.push(Entry::New(new[j].clone(), None));
                j += 1;
            }
            if oi < old.len() {
                entries.push(Entry::Kept(oi));
                i = oi + 1;
                j = nj + 1;
            }
        }
        let slots: Vec<Slot> = arr
            .elements
            .iter()
            .map(|e| Slot {
                start: e.start(),
                end: e.end(),
            })
            .collect();
        self.container(arr.start(), arr.end(), &slots, &entries);
        Some(())
    }

    /// The edits of a container from `open` to `close` whose slots are
    /// `slots`, to hold `entries`: each run of slots that goes is cut with
    /// one separator, and each new entry follows the kept slot before it,
    /// with the separator and the indent of the slots.
    // sokf-implements: KIT-10_AC-1
    #[expect(
        clippy::too_many_lines,
        reason = "one pass over a container, whose removals and insertions share its layout"
    )]
    fn container(&mut self, open: usize, close: usize, slots: &[Slot], entries: &[Entry]) {
        let kept: Vec<usize> = entries
            .iter()
            .filter_map(|e| match e {
                Entry::Kept(i) => Some(*i),
                Entry::New(..) => None,
            })
            .collect();
        // An empty container is written as a pretty one.
        let multiline = slots
            .first()
            .is_none_or(|first| self.text[open + 1..first.start].contains('\n'));
        let indent = match slots.first() {
            Some(first) if multiline => line_indent(self.text, first.start).to_string(),
            _ => format!("{}{}", line_indent(self.text, open), self.unit),
        };
        let sep = match slots {
            [a, b, ..] if is_separator(&self.text[a.end..b.start]) => {
                self.text[a.end..b.start].to_string()
            }
            _ if multiline => format!(",{}{indent}", self.eol),
            _ => ", ".to_string(),
        };
        let rendered: Vec<Option<String>> = entries
            .iter()
            .map(|e| match e {
                Entry::Kept(_) => None,
                Entry::New(value, key) => {
                    let value = if multiline {
                        pretty(value, &indent, self.unit, self.eol)
                    } else {
                        compact(value)
                    };
                    Some(match key {
                        Some(key) => format!("{}: {value}", Value::String(key.clone())),
                        None => value,
                    })
                }
            })
            .collect();
        if kept.is_empty() {
            // Nothing of the container stays: its inside is written anew.
            let items: Vec<&String> = rendered.iter().flatten().collect();
            let body = if items.is_empty() {
                String::new()
            } else if multiline {
                let close_indent = line_indent(self.text, open);
                let eol = self.eol;
                let items: Vec<&str> = items.iter().map(|s| s.as_str()).collect();
                format!(
                    "{eol}{indent}{}{eol}{close_indent}",
                    items.join(&format!(",{eol}{indent}"))
                )
            } else {
                items
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            self.edits.push((open + 1, close - 1, body));
            return;
        }
        // The slots that go: a run before a kept slot goes up to it, and a
        // run at the end goes from the kept slot before it.
        let mut k = 0;
        while k < slots.len() {
            if kept.contains(&k) {
                k += 1;
                continue;
            }
            let run = k;
            while k < slots.len() && !kept.contains(&k) {
                k += 1;
            }
            if k < slots.len() {
                self.edits
                    .push((slots[run].start, slots[k].start, String::new()));
            } else {
                let before = kept.iter().copied().filter(|&i| i < run).max();
                let from = before.map_or(slots[run].start, |i| slots[i].end);
                self.edits.push((from, slots[k - 1].end, String::new()));
            }
        }
        // The new entries: those before the first kept slot go in front of
        // it, the others after the kept slot before them.
        let mut after: Option<usize> = None;
        let mut front = String::new();
        let mut behind = String::new();
        for (e, text) in entries.iter().zip(&rendered) {
            match (e, text) {
                (Entry::Kept(i), _) => {
                    if let Some(at) = after {
                        self.insert(slots[at].end, std::mem::take(&mut behind));
                    }
                    after = Some(*i);
                }
                (Entry::New(..), None) => {}
                (Entry::New(..), Some(text)) if after.is_none() => {
                    front.push_str(text);
                    front.push_str(&sep);
                }
                (Entry::New(..), Some(text)) => {
                    behind.push_str(&sep);
                    behind.push_str(text);
                }
            }
        }
        if let Some(at) = after {
            self.insert(slots[at].end, behind);
        }
        if let Some(&first) = kept.first() {
            self.insert(slots[first].start, front);
        }
    }

    fn insert(&mut self, at: usize, text: String) {
        if !text.is_empty() {
            self.edits.push((at, at, text));
        }
    }
}

/// Whether the tree `ast` holds `value` member for member and item for
/// item, which an object with a duplicate key anywhere breaks.
fn matches(ast: &ast::Value<'_>, value: &Value) -> bool {
    match (ast, value) {
        (ast::Value::Object(obj), Value::Object(map)) => {
            obj.properties.len() == map.len()
                && obj.properties.iter().all(|p| {
                    map.get(p.name.as_str())
                        .is_some_and(|v| matches(&p.value, v))
                })
        }
        (ast::Value::Array(arr), Value::Array(items)) => {
            arr.elements.len() == items.len()
                && arr.elements.iter().zip(items).all(|(e, v)| matches(e, v))
        }
        (ast::Value::Object(_) | ast::Value::Array(_), _)
        | (_, Value::Object(_) | Value::Array(_)) => false,
        _ => true,
    }
}

/// Whether `text` between two siblings is a comma and whitespace alone.
fn is_separator(text: &str) -> bool {
    text.trim() == ","
}

/// The leading whitespace of the line that holds byte `at`.
fn line_indent(text: &str, at: usize) -> &str {
    let start = text[..at].rfind('\n').map_or(0, |i| i + 1);
    let line = &text[start..];
    &line[..line.len() - line.trim_start_matches([' ', '\t']).len()]
}

/// Whether two values are containers of one kind, which an edit in place
/// turns one into the other.
fn same_kind(a: &Value, b: &Value) -> bool {
    matches!(
        (a, b),
        (Value::Object(_), Value::Object(_)) | (Value::Array(_), Value::Array(_))
    )
}

/// The pairs of indexes of a longest common subsequence of `old` and
/// `new`.
fn common(old: &[Value], new: &[Value]) -> Vec<(usize, usize)> {
    let mut len = vec![vec![0usize; new.len() + 1]; old.len() + 1];
    for i in (0..old.len()).rev() {
        for j in (0..new.len()).rev() {
            len[i][j] = if old[i] == new[j] {
                len[i + 1][j + 1] + 1
            } else {
                len[i + 1][j].max(len[i][j + 1])
            };
        }
    }
    let (mut i, mut j, mut pairs) = (0, 0, Vec::new());
    while i < old.len() && j < new.len() {
        if old[i] == new[j] {
            pairs.push((i, j));
            i += 1;
            j += 1;
        } else if len[i + 1][j] >= len[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    pairs
}

/// `value` as pretty JSON with the indent `unit`, its lines after the
/// first indented by `indent`, the indent of the line that it starts on, and
/// ended by `eol`.
fn pretty(value: &Value, indent: &str, unit: &str, eol: &str) -> String {
    super::json::json_text(value, unit, false).replace('\n', &format!("{eol}{indent}"))
}

/// `value` on one line, with a space after each `,` and `:`.
fn compact(value: &Value) -> String {
    match value {
        Value::Array(items) => {
            let items: Vec<String> = items.iter().map(compact).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Object(members) => {
            let members: Vec<String> = members
                .iter()
                .map(|(k, v)| format!("{}: {}", Value::String(k.clone()), compact(v)))
                .collect();
            format!("{{{}}}", members.join(", "))
        }
        _ => value.to_string(),
    }
}

#[cfg(test)]
pub(super) mod tests;
