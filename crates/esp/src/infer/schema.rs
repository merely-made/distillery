// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

//! JSON Schema in front of [`OutputGrammar`] (decision-driver D7).
//!
//! A bounded subset compiles into the existing grammar variants, so replay,
//! forced spans and private-text clearing are unchanged. Output is compact
//! JSON with object members in sorted name order. Every document the grammar
//! accepts satisfies the schema; the reverse is narrower in one place: a
//! string is nonempty and `maxLength` bounds its UTF-8 bytes, not its
//! characters. Anything outside the subset is refused by keyword.
use super::{InferError, OutputGrammar};
use serde_json::{Map, Value};

/// Nesting the grammar compiler accepts (`decoder::constraint`).
const MAX_DEPTH: usize = 24;
const MAX_ALTERNATIVES: usize = 64;
const ANNOTATIONS: &[&str] = &["description", "title"];

/// Compile `schema` into a grammar whose accepted documents all satisfy it.
pub fn grammar_from_schema(schema: &Value) -> Result<OutputGrammar, InferError> {
    let grammar = compile(schema, "$")?;
    if depth(&grammar) > MAX_DEPTH {
        return Err(unsupported("$", "nests deeper than the grammar bound"));
    }
    Ok(grammar)
}

/// Check `value` against `schema` under the rules [`grammar_from_schema`]
/// compiles. The error names the first failing path.
pub fn validate_against_schema(schema: &Value, value: &Value) -> Result<(), InferError> {
    grammar_from_schema(schema)?;
    check(schema, value, "$").map_err(InferError::InvalidRequest)
}

fn unsupported(path: &str, detail: impl Into<String>) -> InferError {
    InferError::Unsupported {
        feature: format!("output_schema {path}"),
        detail: detail.into(),
    }
}

fn invalid(path: &str, detail: &str) -> InferError {
    InferError::InvalidRequest(format!("output schema {path}: {detail}"))
}

fn lit(value: impl Into<String>) -> OutputGrammar {
    OutputGrammar::Literal {
        value: value.into(),
    }
}

/// A flat sequence with adjacent literals merged, so syntax stays one span.
fn seq(parts: Vec<OutputGrammar>) -> OutputGrammar {
    let mut items: Vec<OutputGrammar> = Vec::new();
    let flat = parts.into_iter().flat_map(|part| match part {
        OutputGrammar::Sequence { items } => items,
        other => vec![other],
    });
    for part in flat {
        if let OutputGrammar::Literal { value } = &part {
            if value.is_empty() {
                continue;
            }
            if let Some(OutputGrammar::Literal { value: last }) = items.last_mut() {
                last.push_str(value);
                continue;
            }
        }
        items.push(part);
    }
    match items.len() {
        0 => lit(""),
        1 => items.pop().unwrap(),
        _ => OutputGrammar::Sequence { items },
    }
}

fn choice(path: &str, parts: Vec<OutputGrammar>) -> Result<OutputGrammar, InferError> {
    let mut alternatives = Vec::new();
    for part in parts {
        match part {
            OutputGrammar::Choice { alternatives: inner }
                if alternatives.len() + inner.len() <= MAX_ALTERNATIVES =>
            {
                alternatives.extend(inner);
            }
            other => alternatives.push(other),
        }
    }
    match alternatives.len() {
        0 => Err(invalid(path, "no alternatives")),
        1 => Ok(alternatives.pop().unwrap()),
        n if n > MAX_ALTERNATIVES => Err(unsupported(path, "more than 64 alternatives")),
        _ => Ok(OutputGrammar::Choice { alternatives }),
    }
}

fn depth(grammar: &OutputGrammar) -> usize {
    match grammar {
        OutputGrammar::Literal { .. } | OutputGrammar::JsonString { .. } => 0,
        OutputGrammar::Sequence { items } => 1 + items.iter().map(depth).max().unwrap_or(0),
        OutputGrammar::Choice { alternatives } => {
            1 + alternatives.iter().map(depth).max().unwrap_or(0)
        }
    }
}

/// Refuse any keyword outside `allowed` (annotations aside) by name.
fn only(object: &Map<String, Value>, path: &str, allowed: &[&str]) -> Result<(), InferError> {
    match object
        .keys()
        .find(|key| !allowed.contains(&key.as_str()) && !ANNOTATIONS.contains(&key.as_str()))
    {
        Some(key) => Err(unsupported(path, format!("keyword {key:?}"))),
        None => Ok(()),
    }
}

/// The JSON text of a scalar `const` or `enum` member.
fn scalar(value: &Value, path: &str) -> Result<String, InferError> {
    let integral = value.as_number().is_none_or(|n| n.is_i64() || n.is_u64());
    if value.is_array() || value.is_object() || !integral {
        return Err(unsupported(path, "only string, integer, boolean and null constants"));
    }
    serde_json::to_string(value).map_err(|error| invalid(path, &error.to_string()))
}

fn bound(object: &Map<String, Value>, key: &str, path: &str) -> Result<Option<usize>, InferError> {
    match object.get(key) {
        None => Ok(None),
        Some(value) => value
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .map(Some)
            .ok_or_else(|| invalid(path, &format!("{key} must be a non-negative integer"))),
    }
}

fn compile(schema: &Value, path: &str) -> Result<OutputGrammar, InferError> {
    let object = schema
        .as_object()
        .ok_or_else(|| invalid(path, "a schema must be an object"))?;
    if let Some(value) = object.get("const") {
        only(object, path, &["const"])?;
        return scalar(value, path).map(lit);
    }
    if let Some(values) = object.get("enum") {
        only(object, path, &["enum"])?;
        let values = values
            .as_array()
            .ok_or_else(|| invalid(path, "enum must be an array"))?;
        let literals = values
            .iter()
            .map(|value| scalar(value, path).map(lit))
            .collect::<Result<Vec<_>, _>>()?;
        return choice(path, literals);
    }
    if let Some(alternatives) = object.get("anyOf") {
        only(object, path, &["anyOf"])?;
        let alternatives = alternatives
            .as_array()
            .ok_or_else(|| invalid(path, "anyOf must be an array"))?;
        let compiled = alternatives
            .iter()
            .enumerate()
            .map(|(index, schema)| compile(schema, &format!("{path}.anyOf[{index}]")))
            .collect::<Result<Vec<_>, _>>()?;
        return choice(path, compiled);
    }
    let Some(kind) = object.get("type") else {
        let key = object.keys().next().map_or("nothing", String::as_str);
        return Err(unsupported(path, format!("no type, const, enum or anyOf (found {key:?})")));
    };
    match kind.as_str() {
        Some("boolean") => {
            only(object, path, &["type"])?;
            choice(path, vec![lit("true"), lit("false")])
        }
        Some("null") => {
            only(object, path, &["type"])?;
            Ok(lit("null"))
        }
        Some("string") => {
            only(object, path, &["type", "maxLength", "minLength"])?;
            let max_bytes = bound(object, "maxLength", path)?
                .filter(|max| (1..=4096).contains(max))
                .ok_or_else(|| invalid(path, "a string needs maxLength in 1..=4096"))?;
            if bound(object, "minLength", path)?.is_some_and(|min| min > 1) {
                return Err(unsupported(path, "minLength above 1"));
            }
            Ok(OutputGrammar::JsonString { max_bytes })
        }
        Some("integer") => {
            only(object, path, &["type", "minimum", "maximum"])?;
            let end = |key: &str| {
                object
                    .get(key)
                    .and_then(Value::as_i64)
                    .ok_or_else(|| invalid(path, &format!("an integer needs an integer {key}")))
            };
            let (low, high) = (end("minimum")?, end("maximum")?);
            if low > high {
                return Err(invalid(path, "minimum exceeds maximum"));
            }
            integers(path, i128::from(low), i128::from(high))
        }
        Some("array") => {
            only(object, path, &["type", "items", "maxItems", "minItems"])?;
            let item = object
                .get("items")
                .ok_or_else(|| invalid(path, "an array needs items"))?;
            let item = compile(item, &format!("{path}[]"))?;
            let max = bound(object, "maxItems", path)?
                .filter(|max| *max >= 1)
                .ok_or_else(|| invalid(path, "an array needs maxItems of at least 1"))?;
            let min = bound(object, "minItems", path)?.unwrap_or(0);
            if min > max {
                return Err(invalid(path, "minItems exceeds maxItems"));
            }
            // Unrolled from the last slot back: after `placed` items the list
            // closes, or takes one more where the bounds allow it.
            let mut rest = lit("]");
            for placed in (1..max).rev() {
                let more = seq(vec![lit(","), item.clone(), rest]);
                rest = if placed >= min {
                    choice(path, vec![lit("]"), more])?
                } else {
                    more
                };
            }
            let first = seq(vec![item, rest]);
            let body = if min == 0 {
                choice(path, vec![lit("]"), first])?
            } else {
                first
            };
            Ok(seq(vec![lit("["), body]))
        }
        Some("object") => {
            only(
                object,
                path,
                &["type", "properties", "required", "additionalProperties"],
            )?;
            if object.get("additionalProperties") != Some(&Value::Bool(false)) {
                return Err(unsupported(path, "an object must set additionalProperties to false"));
            }
            let empty = Map::new();
            let properties = match object.get("properties") {
                None => &empty,
                Some(value) => value
                    .as_object()
                    .ok_or_else(|| invalid(path, "properties must be an object"))?,
            };
            let mut names: Vec<&String> = properties.keys().collect();
            names.sort_unstable();
            let mut required: Vec<&str> = object
                .get("required")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            required.sort_unstable();
            if required != names.iter().map(|name| name.as_str()).collect::<Vec<_>>() {
                return Err(unsupported(path, "every property must be required"));
            }
            let mut parts = vec![lit("{")];
            for (index, name) in names.into_iter().enumerate() {
                let key = serde_json::to_string(name)
                    .map_err(|error| invalid(path, &error.to_string()))?;
                parts.push(lit(format!("{}{key}:", if index == 0 { "" } else { "," })));
                parts.push(compile(&properties[name], &format!("{path}.{name}"))?);
            }
            parts.push(lit("}"));
            Ok(seq(parts))
        }
        Some(other) => Err(unsupported(path, format!("type {other:?}"))),
        None => Err(unsupported(path, "type must be one string")),
    }
}

/// Exactly the decimal integers in `low..=high`, with no leading zeros.
fn integers(path: &str, low: i128, high: i128) -> Result<OutputGrammar, InferError> {
    let mut parts = Vec::new();
    if low < 0 {
        let nearest = if high < 0 { -high } else { 1 };
        parts.push(seq(vec![lit("-"), naturals(path, nearest as u128, (-low) as u128)?]));
    }
    if high >= 0 {
        parts.push(naturals(path, low.max(0) as u128, high as u128)?);
    }
    choice(path, parts)
}

fn naturals(path: &str, low: u128, high: u128) -> Result<OutputGrammar, InferError> {
    let width = |n: u128| n.to_string().len();
    let mut parts = Vec::new();
    for digits in width(low)..=width(high) {
        let floor = if digits == 1 { 0 } else { 10u128.pow(digits as u32 - 1) };
        let (from, to) = (low.max(floor), high.min(10u128.pow(digits as u32) - 1));
        if from <= to {
            parts.push(between(path, from.to_string().as_bytes(), to.to_string().as_bytes())?);
        }
    }
    choice(path, parts)
}

fn digit_run(path: &str, from: u8, to: u8) -> Result<OutputGrammar, InferError> {
    choice(path, (from..=to).map(|digit| lit(char::from(digit))).collect())
}

/// Digit strings of one width between `low` and `high`, both inclusive.
fn between(path: &str, low: &[u8], high: &[u8]) -> Result<OutputGrammar, InferError> {
    let text = |digits: &[u8]| lit(String::from_utf8_lossy(digits).into_owned());
    if low == high {
        return Ok(text(low));
    }
    if low[0] == high[0] {
        return Ok(seq(vec![text(&low[..1]), between(path, &low[1..], &high[1..])?]));
    }
    let width = low.len() - 1;
    let (mut from, mut to) = (low[0], high[0]);
    let mut parts = Vec::new();
    if low[1..].iter().any(|digit| *digit != b'0') {
        let ceiling = vec![b'9'; width];
        parts.push(seq(vec![text(&low[..1]), between(path, &low[1..], &ceiling)?]));
        from += 1;
    }
    let upper = if high[1..].iter().any(|digit| *digit != b'9') {
        let floor = vec![b'0'; width];
        to -= 1;
        Some(seq(vec![text(&high[..1]), between(path, &floor, &high[1..])?]))
    } else {
        None
    };
    if from <= to {
        let mut run = vec![digit_run(path, from, to)?];
        for _ in 0..width {
            run.push(digit_run(path, b'0', b'9')?);
        }
        parts.push(seq(run));
    }
    parts.extend(upper);
    choice(path, parts)
}

fn check(schema: &Value, value: &Value, path: &str) -> Result<(), String> {
    let fail = |detail: &str| Err(format!("decision {path}: {detail}"));
    let object = schema.as_object().expect("schema compiled");
    if let Some(expected) = object.get("const") {
        return if value == expected { Ok(()) } else { fail("is not the constant") };
    }
    if let Some(values) = object.get("enum").and_then(Value::as_array) {
        return if values.contains(value) { Ok(()) } else { fail("is not an enum member") };
    }
    if let Some(alternatives) = object.get("anyOf").and_then(Value::as_array) {
        return if alternatives.iter().any(|schema| check(schema, value, path).is_ok()) {
            Ok(())
        } else {
            fail("matches no alternative")
        };
    }
    let size = |key: &str| object.get(key).and_then(Value::as_u64).map(|n| n as usize);
    match object.get("type").and_then(Value::as_str) {
        Some("boolean") if value.is_boolean() => Ok(()),
        Some("null") if value.is_null() => Ok(()),
        Some("string") => match value.as_str() {
            Some(text) if !text.is_empty() && text.len() <= size("maxLength").unwrap_or(0) => {
                Ok(())
            }
            Some(_) => fail("string is empty or longer than maxLength bytes"),
            None => fail("is not a string"),
        },
        Some("integer") => {
            let end = |key: &str| object.get(key).and_then(Value::as_i64);
            match value.as_i64().filter(|_| !value.is_f64()) {
                Some(n) if Some(n) >= end("minimum") && Some(n) <= end("maximum") => Ok(()),
                Some(_) => fail("integer is out of range"),
                None => fail("is not an integer"),
            }
        }
        Some("array") => {
            let Some(items) = value.as_array() else {
                return fail("is not an array");
            };
            if items.len() < size("minItems").unwrap_or(0)
                || items.len() > size("maxItems").unwrap_or(0)
            {
                return fail("array length is out of bounds");
            }
            items.iter().enumerate().try_for_each(|(index, item)| {
                check(&object["items"], item, &format!("{path}[{index}]"))
            })
        }
        Some("object") => {
            let Some(members) = value.as_object() else {
                return fail("is not an object");
            };
            let empty = Map::new();
            let properties = object
                .get("properties")
                .and_then(Value::as_object)
                .unwrap_or(&empty);
            if members.len() != properties.len() {
                return fail("object members differ from the declared properties");
            }
            properties.iter().try_for_each(|(name, schema)| {
                let path = format!("{path}.{name}");
                match members.get(name) {
                    Some(member) => check(schema, member, &path),
                    None => Err(format!("decision {path}: is missing")),
                }
            })
        }
        _ => fail("has the wrong type"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Every document a string-free grammar accepts; strings stand in as "x".
    fn expand(grammar: &OutputGrammar) -> Vec<String> {
        match grammar {
            OutputGrammar::Literal { value } => vec![value.clone()],
            OutputGrammar::JsonString { .. } => vec!["\"x\"".into()],
            OutputGrammar::Choice { alternatives } => {
                alternatives.iter().flat_map(expand).collect()
            }
            OutputGrammar::Sequence { items } => items.iter().fold(vec![String::new()], |done, item| {
                let tails = expand(item);
                done.iter()
                    .flat_map(|head| tails.iter().map(move |tail| format!("{head}{tail}")))
                    .collect()
            }),
        }
    }

    #[test]
    fn integer_ranges_are_exact_and_unambiguous() {
        for (low, high) in [
            (0i64, 0i64),
            (0, 9),
            (7, 23),
            (-12, 105),
            (-5, -3),
            (99, 1001),
            (-1000, -99),
            (-2048, 65535),
        ] {
            let schema = json!({"type": "integer", "minimum": low, "maximum": high});
            let mut got = expand(&grammar_from_schema(&schema).unwrap());
            got.sort();
            let mut want: Vec<String> = (low..=high).map(|n| n.to_string()).collect();
            want.sort();
            assert_eq!(got, want, "{low}..={high}");
        }
    }

    #[test]
    fn every_accepted_operation_parses_and_validates() {
        let schema = json!({"anyOf": [
            {"type": "object", "additionalProperties": false,
             "required": ["op", "passage", "weight", "tags", "note"],
             "properties": {
                "op": {"const": "cite"},
                "passage": {"type": "integer", "minimum": 0, "maximum": 11},
                "weight": {"enum": [1, 2, null]},
                "tags": {"type": "array", "maxItems": 2, "items": {"enum": ["a", "b"]}},
                "note": {"type": "string", "maxLength": 8}}},
            {"type": "object", "additionalProperties": false,
             "required": ["op", "done"],
             "properties": {"op": {"const": "stop"}, "done": {"type": "boolean"}}}
        ]});
        let documents = expand(&grammar_from_schema(&schema).unwrap());
        // 12 passages x 3 weights x (1 + 2 + 4) tag lists, plus 2 stops.
        assert_eq!(documents.len(), 12 * 3 * 7 + 2);
        assert!(documents.contains(&r#"{"done":true,"op":"stop"}"#.to_string()));
        for document in documents {
            let value: Value = serde_json::from_str(&document).expect(&document);
            validate_against_schema(&schema, &value).expect(&document);
        }
    }

    #[test]
    fn array_bounds_are_exact() {
        let lists = |min: usize| {
            let schema = json!({"type": "array", "items": {"type": "boolean"},
                                "minItems": min, "maxItems": 3});
            expand(&grammar_from_schema(&schema).unwrap())
        };
        assert_eq!(lists(0).len(), 1 + 2 + 4 + 8);
        assert_eq!(lists(1).len(), 2 + 4 + 8);
        assert_eq!(lists(3).len(), 8);
        assert!(lists(0).contains(&"[]".to_string()));
        assert!(lists(3).contains(&"[true,false,true]".to_string()));
    }

    #[test]
    fn what_is_outside_the_subset_is_refused_by_name() {
        for (schema, word) in [
            (json!({"type": "number"}), "number"),
            (json!({"type": "string"}), "maxLength"),
            (json!({"type": "string", "maxLength": 4, "pattern": "a+"}), "pattern"),
            (json!({"type": "integer", "minimum": 0}), "maximum"),
            (json!({"type": "integer", "minimum": 0.5, "maximum": 2}), "minimum"),
            (json!({"type": "array", "items": {"type": "null"}}), "maxItems"),
            (json!({"type": "object", "properties": {}}), "additionalProperties"),
            (json!({"type": "object", "additionalProperties": false,
                    "properties": {"a": {"type": "null"}}}), "required"),
            (json!({"oneOf": [{"type": "null"}]}), "oneOf"),
            (json!({"$ref": "#/x"}), "$ref"),
            (json!({"const": 1.5}), "constants"),
            (json!({"enum": []}), "alternatives"),
        ] {
            let error = format!("{:?}", grammar_from_schema(&schema).unwrap_err());
            assert!(error.contains(word), "{schema}: {error}");
        }
        let mut deep = json!({"type": "boolean"});
        for _ in 0..13 {
            deep = json!({"type": "array", "maxItems": 2, "items": deep});
        }
        assert!(grammar_from_schema(&deep).is_err());
    }

    #[test]
    fn decisions_outside_the_schema_are_refused_by_path() {
        let schema = json!({"type": "object", "additionalProperties": false,
            "required": ["n", "s"],
            "properties": {"n": {"type": "integer", "minimum": 1, "maximum": 3},
                           "s": {"type": "string", "maxLength": 2}}});
        validate_against_schema(&schema, &json!({"n": 2, "s": "ok"})).unwrap();
        for (value, word) in [
            (json!({"n": 4, "s": "ok"}), "$.n"),
            (json!({"n": 2.0, "s": "ok"}), "$.n"),
            (json!({"n": 2, "s": ""}), "$.s"),
            (json!({"n": 2, "s": "é!"}), "$.s"),
            (json!({"n": 2}), "members"),
            (json!({"n": 2, "s": "ok", "x": 1}), "members"),
            (json!([2, "ok"]), "not an object"),
        ] {
            let error = format!("{:?}", validate_against_schema(&schema, &value).unwrap_err());
            assert!(error.contains(word), "{value}: {error}");
        }
    }
}
