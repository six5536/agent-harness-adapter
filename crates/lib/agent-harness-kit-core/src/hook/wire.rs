//! The hook contract (AHK-2): the neutral input a tool reads on stdin and
//! the answer it writes on stdout, as JSON. `ahk hook` and the language
//! bindings speak it, so a tool in any language sees one format.
// @zen-component: AHK-Wire

use serde_json::Value;

use crate::{
    Error, Result,
    hook::{Answer, HookInput},
};

/// The contract's version, `v` in the input and the answer.
pub const VERSION: u64 = 1;

const ANSWER: &str = "hook answer";

/// `input` as the contract's input: its JSON with `"v": 1` first.
// @zen-impl: AHK-2_AC-1
pub fn input_json(input: &HookInput) -> Value {
    let mut out = serde_json::Map::new();
    out.insert("v".into(), VERSION.into());
    if let Value::Object(fields) = serde_json::to_value(input).expect("a HookInput serialises") {
        out.extend(fields);
    }
    Value::Object(out)
}

/// The answer in `text`. Unknown fields are ignored; a `v` other than 1 is
/// an error, as is anything that is not an answer.
// @zen-impl: AHK-2_AC-2
pub fn parse_answer(text: &str) -> Result<Answer> {
    let value: Value =
        serde_json::from_str(text.trim()).map_err(|e| Error::file(ANSWER, e.to_string()))?;
    match value.get("v") {
        None => {}
        Some(v) if v.as_u64() == Some(VERSION) => {}
        Some(v) => {
            return Err(Error::file(
                ANSWER,
                format!("version {v} is not one this kit reads (1)"),
            ));
        }
    }
    serde_json::from_value(value).map_err(|e| Error::file(ANSWER, e.to_string()))
}

/// `answer` as the contract's answer, with `"v": 1`.
pub fn answer_json(answer: &Answer) -> Value {
    let mut out = serde_json::Map::new();
    out.insert("v".into(), VERSION.into());
    if let Value::Object(fields) = serde_json::to_value(answer).expect("an Answer serialises") {
        out.extend(fields);
    }
    Value::Object(out)
}

/// The JSON Schema of the contract's input.
#[cfg(feature = "schemars")]
pub fn input_schema() -> Value {
    with_version(schemars::schema_for!(HookInput), "AHK hook input", true)
}

/// The JSON Schema of the contract's answer.
#[cfg(feature = "schemars")]
pub fn answer_schema() -> Value {
    with_version(schemars::schema_for!(Answer), "AHK hook answer", false)
}

/// `schema` with the property `v` (required when `required`) and `title`.
#[cfg(feature = "schemars")]
fn with_version(schema: schemars::Schema, title: &str, required: bool) -> Value {
    let mut schema = serde_json::to_value(schema).expect("a schema serialises");
    schema["title"] = title.into();
    let v = serde_json::json!({ "description": "The contract's version.", "const": VERSION });
    // An answer is one of several shapes: `v` goes on each.
    let shapes: Vec<&mut Value> = match schema.get_mut("oneOf") {
        Some(Value::Array(all)) => all.iter_mut().collect(),
        _ => vec![&mut schema],
    };
    for s in shapes {
        s["properties"]["v"] = v.clone();
        if required {
            match s.get_mut("required") {
                Some(Value::Array(r)) => r.insert(0, "v".into()),
                _ => s["required"] = serde_json::json!(["v"]),
            }
        }
    }
    schema
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use serde_json::json;

    use super::*;
    use crate::hook::{Event, ToolCall, ToolKind};

    // @zen-test: AHK-2_AC-1
    #[test]
    fn the_input_leaves_out_what_is_absent() {
        let bare = HookInput::new("claude", Event::Stop, Value::Null);
        assert_eq!(
            input_json(&bare),
            json!({"v": 1, "harness": "claude", "event": "stop"})
        );
        let mut full = HookInput::new("codex", Event::PreTool, json!({"x": 1}));
        full.session_id = Some("s".into());
        full.tool = Some(ToolCall::new(
            "shell",
            ToolKind::Shell,
            json!({"cmd": "ls"}),
        ));
        full.continuing = true;
        assert_eq!(
            input_json(&full),
            json!({
                "v": 1, "harness": "codex", "event": "pre-tool", "session_id": "s",
                "tool": {"name": "shell", "kind": "shell", "input": {"cmd": "ls"}},
                "continuing": true, "raw": {"x": 1}
            })
        );
        let back: HookInput = serde_json::from_value(input_json(&full)).unwrap();
        assert_eq!(back, full);
    }

    // @zen-test: AHK-2_AC-2
    #[test]
    fn answers_parse_from_the_contract() {
        assert_eq!(
            parse_answer(r#"{"answer":"allow"}"#).unwrap(),
            Answer::Allow { stderr: None }
        );
        assert_eq!(
            parse_answer(r#" {"v":1,"answer":"deny","reason":"no","extra":true} "#).unwrap(),
            Answer::Deny {
                reason: "no".into()
            }
        );
        assert_eq!(
            parse_answer(r#"{"answer":"context","text":"t"}"#).unwrap(),
            Answer::Context { text: "t".into() }
        );
        for (bad, says) in [
            (r#"{"v":2,"answer":"allow"}"#, "version 2"),
            (r#"{"answer":"maybe"}"#, "unknown variant"),
            (r#"{"answer":"deny"}"#, "missing field `reason`"),
            ("not json", "expected"),
        ] {
            let e = parse_answer(bad).unwrap_err().to_string();
            assert!(e.contains("hook answer") && e.contains(says), "{bad}: {e}");
        }
    }

    // @zen-test: AHK-2_AC-3
    #[test]
    fn pi_answers_in_the_contract() {
        let pi = crate::harness::find("pi").unwrap();
        for (event, answer) in [
            (Event::Stop, Answer::Allow { stderr: None }),
            (Event::PreTool, Answer::Deny { reason: "r".into() }),
            (Event::Stop, Answer::Continue { reason: "r".into() }),
            (Event::PostTool, Answer::Context { text: "t".into() }),
        ] {
            let out = pi.answer(event, &answer).unwrap();
            assert_eq!(parse_answer(&out.stdout).unwrap(), answer);
        }
    }

    fn answers() -> impl Strategy<Value = Answer> {
        prop_oneof![
            proptest::option::of(".*").prop_map(|stderr| Answer::Allow { stderr }),
            ".*".prop_map(|reason| Answer::Deny { reason }),
            ".*".prop_map(|reason| Answer::Continue { reason }),
            ".*".prop_map(|text| Answer::Context { text }),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]
        // @zen-test: AHK_P-2
        #[test]
        fn answers_round_trip(a in answers()) {
            let text = answer_json(&a).to_string();
            prop_assert_eq!(parse_answer(&text).unwrap(), a);
        }
    }

    #[cfg(feature = "schemars")]
    #[test]
    fn the_schemas_carry_the_version() {
        let input = input_schema();
        assert_eq!(input["title"], "AHK hook input");
        assert_eq!(input["properties"]["v"]["const"], 1);
        assert_eq!(input["required"][0], "v");
        let answer = answer_schema();
        let shapes = answer["oneOf"].as_array().unwrap();
        assert_eq!(shapes.len(), 4);
        assert!(shapes.iter().all(|s| s["properties"]["v"]["const"] == 1));
        assert!(shapes.iter().all(|s| s.get("required").is_some()));
    }
}
