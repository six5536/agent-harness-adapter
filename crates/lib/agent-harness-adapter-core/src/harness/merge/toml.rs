//! The TOML merge: object members set in the user's TOML file with
//! `toml_edit`, keeping its comments, order and formatting.

use serde_json::{Map, Number, Value};
use toml_edit::{Array, DocumentMut, InlineTable, Item, Table};

use crate::{
    Error, Result,
    harness::merge::{MergeOp, op::Op},
};

/// Parse TOML, keeping its formatting; `display` names the file in the
/// refusal.
pub(crate) fn parse_toml(display: &str, text: &str) -> Result<DocumentMut> {
    text.parse()
        .map_err(|e: toml_edit::TomlError| Error::file(display, e.message()))
}

/// The JSON form of a TOML value, for comparison and hashing; dates as
/// strings.
fn to_json(item: &Item) -> Option<Value> {
    Some(match item {
        Item::None => return None,
        Item::Value(v) => value_to_json(v),
        Item::Table(t) => Value::Object(
            t.iter()
                .filter_map(|(k, v)| Some((k.to_string(), to_json(v)?)))
                .collect(),
        ),
        Item::ArrayOfTables(a) => Value::Array(
            a.iter()
                .map(|t| to_json(&Item::Table(t.clone())).unwrap_or(Value::Null))
                .collect(),
        ),
    })
}

fn value_to_json(v: &toml_edit::Value) -> Value {
    match v {
        toml_edit::Value::String(s) => Value::String(s.value().clone()),
        toml_edit::Value::Integer(i) => Value::from(*i.value()),
        toml_edit::Value::Float(f) => {
            Number::from_f64(*f.value()).map_or(Value::Null, Value::Number)
        }
        toml_edit::Value::Boolean(b) => Value::Bool(*b.value()),
        toml_edit::Value::Datetime(d) => Value::String(d.value().to_string()),
        toml_edit::Value::Array(a) => Value::Array(a.iter().map(value_to_json).collect()),
        toml_edit::Value::InlineTable(t) => Value::Object(
            t.iter()
                .map(|(k, v)| (k.to_string(), value_to_json(v)))
                .collect::<Map<_, _>>(),
        ),
    }
}

/// A JSON value as a TOML value: objects as inline tables; `null` has no
/// TOML form.
fn from_json(v: &Value) -> Result<toml_edit::Value> {
    Ok(match v {
        Value::Null => return Err(Error::Internal("TOML has no null".into())),
        Value::Bool(b) => (*b).into(),
        Value::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => i.into(),
            (None, Some(f)) => f.into(),
            _ => return Err(Error::Internal(format!("no TOML number for {n}"))),
        },
        Value::String(s) => s.as_str().into(),
        Value::Array(a) => {
            let mut out = Array::new();
            for x in a {
                out.push(from_json(x)?);
            }
            toml_edit::Value::Array(out)
        }
        Value::Object(m) => {
            let mut t = InlineTable::new();
            for (k, x) in m {
                t.insert(k, from_json(x)?);
            }
            toml_edit::Value::InlineTable(t)
        }
    })
}

/// A JSON object as a TOML table, its nested objects inline; any other value
/// as a TOML value.
fn item_from_json(v: &Value) -> Result<Item> {
    match v {
        Value::Object(m) => {
            let mut t = Table::new();
            for (k, x) in m {
                t.insert(k, Item::Value(from_json(x)?));
            }
            Ok(Item::Table(t))
        }
        other => Ok(Item::Value(from_json(other)?)),
    }
}

/// The table at `path`, created (implicit) along the way when absent.
fn table_at<'a>(doc: &'a mut DocumentMut, path: &[String], display: &str) -> Result<&'a mut Table> {
    let mut cur = doc.as_table_mut();
    for (i, key) in path.iter().enumerate() {
        let item = cur.entry(key).or_insert_with(|| {
            let mut t = Table::new();
            t.set_implicit(true);
            Item::Table(t)
        });
        cur = item.as_table_mut().ok_or_else(|| {
            Error::file(
                display,
                format!("`{}` is not a table", path[..=i].join(".")),
            )
        })?;
    }
    Ok(cur)
}

pub(super) fn members(op: &MergeOp) -> Result<(&[String], &str, &Value)> {
    match &op.0 {
        Op::ObjectMember { path, key, value } => Ok((path, key, value)),
        _ => Err(Error::Internal(
            "a TOML file takes object members only".into(),
        )),
    }
}

/// The member of `op` as found in `doc`, as JSON.
pub(crate) fn extract_toml(doc: &DocumentMut, op: &MergeOp) -> Result<Option<Value>> {
    let (path, key, _) = members(op)?;
    let mut cur = doc.as_item();
    for k in path {
        match cur.get(k.as_str()) {
            Some(i) => cur = i,
            None => return Ok(None),
        }
    }
    Ok(cur.get(key).and_then(to_json))
}

/// The file to write with `ops` merged into `existing` (a new file holds
/// only the tool's tables). `None` when nothing changes.
// @zen-impl: KIT-2_AC-5
// @zen-impl: KIT-10_AC-5
pub(crate) fn render_toml_merge(
    existing: Option<&str>,
    ops: &[MergeOp],
    display: &str,
) -> Result<Option<String>> {
    let mut doc = match existing {
        Some(text) => parse_toml(display, text)?,
        None => DocumentMut::new(),
    };
    // A file of comments only keeps them as trailing text, which would move
    // below the new tables: keep the file's text first instead.
    let lead = match existing {
        Some(text) if doc.as_table().is_empty() => {
            doc = DocumentMut::new();
            let mut t = text.to_string();
            if !t.is_empty() && !t.ends_with('\n') {
                t.push('\n');
            }
            t
        }
        _ => String::new(),
    };
    for op in ops {
        let (path, key, value) = members(op)?;
        if extract_toml(&doc, op)?.as_ref() == Some(value) {
            continue;
        }
        let item = item_from_json(value)?;
        table_at(&mut doc, path, display)?.insert(key, item);
    }
    let out = format!("{lead}{doc}");
    Ok((Some(out.as_str()) != existing).then_some(out))
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use serde_json::json;

    use super::*;

    const P: &str = ".codex/config.toml";

    fn server(name: &str, command: &str) -> MergeOp {
        MergeOp::object_member(
            ["mcp_servers"],
            name,
            json!({ "command": command, "args": ["mcp"], "env": { "A": "1" } }),
        )
    }

    // @zen-test: KIT-2_AC-5
    // @zen-test: KIT-10_AC-5
    #[test]
    fn a_table_is_added_and_the_rest_kept() {
        let out = render_toml_merge(None, &[server("tool", "tool")], P)
            .unwrap()
            .unwrap();
        assert_eq!(
            out,
            "[mcp_servers.tool]\ncommand = \"tool\"\nargs = [\"mcp\"]\nenv = { A = \"1\" }\n"
        );
        let existing =
            "# my config\nmodel = \"o3\" # pinned\n\n[mcp_servers.other]\ncommand = \"x\"\n";
        let out = render_toml_merge(Some(existing), &[server("tool", "tool")], P)
            .unwrap()
            .unwrap();
        assert!(out.starts_with(existing), "{out}");
        let doc = parse_toml(P, &out).unwrap();
        assert_eq!(
            extract_toml(&doc, &server("tool", "tool")).unwrap(),
            Some(json!({ "command": "tool", "args": ["mcp"], "env": { "A": "1" } }))
        );
        assert_eq!(
            render_toml_merge(Some(&out), &[server("tool", "tool")], P).unwrap(),
            None
        );
        // A changed value is replaced in place.
        let out2 = render_toml_merge(Some(&out), &[server("tool", "tool2")], P)
            .unwrap()
            .unwrap();
        assert!(out2.contains("command = \"tool2\""), "{out2}");
        assert!(out2.starts_with(existing), "{out2}");
        assert_eq!(
            extract_toml(&parse_toml(P, existing).unwrap(), &server("tool", "t")).unwrap(),
            None
        );
    }

    #[test]
    fn refusals() {
        let e = parse_toml("c.toml", "a = ").unwrap_err();
        assert!(
            matches!(&e, Error::File { file, .. } if file == "c.toml"),
            "{e}"
        );
        let e = render_toml_merge(Some("mcp_servers = 1\n"), &[server("t", "t")], P).unwrap_err();
        assert!(
            e.to_string().contains("`mcp_servers` is not a table"),
            "{e}"
        );
        let e = render_toml_merge(None, &[MergeOp::array_entry(["a"], 1)], P).unwrap_err();
        assert!(matches!(e, Error::Internal(_)), "{e}");
        let e = render_toml_merge(None, &[MergeOp::object_member(["a"], "b", Value::Null)], P)
            .unwrap_err();
        assert!(matches!(e, Error::Internal(_)), "{e}");
    }

    #[test]
    fn values_convert_both_ways() {
        let v = json!({ "b": true, "i": 3, "f": 1.5, "s": "x", "a": [1, {"k": "v"}] });
        let out = render_toml_merge(
            None,
            &[MergeOp::object_member(Vec::<String>::new(), "t", v.clone())],
            P,
        )
        .unwrap()
        .unwrap();
        let doc = parse_toml(P, &out).unwrap();
        assert_eq!(
            extract_toml(
                &doc,
                &MergeOp::object_member(Vec::<String>::new(), "t", v.clone())
            )
            .unwrap(),
            Some(v)
        );
        let doc = parse_toml(P, "d = 1979-05-27\n[[x]]\na = 1\n").unwrap();
        assert_eq!(
            extract_toml(&doc, &MergeOp::object_member(Vec::<String>::new(), "d", 0)).unwrap(),
            Some(json!("1979-05-27"))
        );
        assert_eq!(
            extract_toml(&doc, &MergeOp::object_member(Vec::<String>::new(), "x", 0)).unwrap(),
            Some(json!([{ "a": 1 }]))
        );
    }

    fn arb_doc() -> impl Strategy<Value = String> {
        (
            prop::collection::btree_map("[a-z]{1,6}", 0i64..100, 0..4),
            prop::option::of("[a-z ]{0,10}"),
            prop::bool::ANY,
        )
            .prop_map(|(keys, comment, other)| {
                let mut out = String::new();
                if let Some(c) = comment {
                    out.push_str(&format!("# {c}\n"));
                }
                for (k, v) in keys {
                    out.push_str(&format!("k{k} = {v}\n"));
                }
                if other {
                    out.push_str("\n[mcp_servers.other] # theirs\ncommand = \"x\"\n");
                }
                out
            })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(128))]

        // @zen-test: KIT_P-11
        #[test]
        fn toml_merge_keeps_the_rest(text in arb_doc(), name in "[a-z]{1,6}") {
            let op = server(&format!("t{name}"), "tool");
            let out = render_toml_merge(Some(&text), std::slice::from_ref(&op), P).unwrap().unwrap();
            prop_assert!(out.starts_with(&text), "{}", out);
            let doc = parse_toml(P, &out).unwrap();
            let found = extract_toml(&doc, &op).unwrap();
            prop_assert_eq!(found.as_ref(), Some(op.value()));
            prop_assert_eq!(render_toml_merge(Some(&out), std::slice::from_ref(&op), P).unwrap(), None);
        }
    }
}
