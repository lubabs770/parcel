//! Tiny JSON paths: "from.0.name", with "a|b" fallbacks.

use serde_json::Value;

/// Look up a dot path in a JSON value; numeric segments index arrays. `a|b`
/// tries each alternative and returns the first non-null, non-empty hit.
/// The empty path returns `v` itself (for CLIs that print a bare array).
pub fn get<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('|').find_map(|alt| {
        let hit = alt.trim().split('.').filter(|s| !s.is_empty()).try_fold(v, |cur, key| match cur {
            Value::Object(m) => m.get(key),
            Value::Array(a) => key.parse::<usize>().ok().and_then(|i| a.get(i)),
            _ => None,
        })?;
        match hit {
            Value::Null => None,
            Value::String(s) if s.is_empty() => None,
            _ => Some(hit),
        }
    })
}

/// `get` as text: strings as-is, anything else via its JSON rendering.
pub fn text(v: &Value, path: &str) -> Option<String> {
    get(v, path).map(|h| match h {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::{get, text};
    use serde_json::json;

    #[test]
    fn dot_paths_indexes_and_fallbacks() {
        let v = json!({"envelopes": [{"id": "7", "from": [{"name": null, "email": "a@x.io"}], "n": 3, "s": ""}]});
        assert_eq!(text(&v, "envelopes.0.id").as_deref(), Some("7"));
        assert_eq!(text(&v, "envelopes.0.from.0.name|envelopes.0.from.0.email").as_deref(), Some("a@x.io"));
        assert_eq!(text(&v, "envelopes.0.n").as_deref(), Some("3"));
        assert!(get(&v, "envelopes.9.id").is_none());
        assert!(get(&v, "envelopes.0.s").is_none(), "empty string counts as missing");
        assert_eq!(get(&v, ""), Some(&v), "empty path is the value itself");
    }
}
