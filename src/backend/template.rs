//! Command templates → argv. No shell is ever involved.

use std::collections::HashMap;

/// Expand a command template into argv. The template is split on whitespace
/// first; each `{name}` is then replaced inside its own argv entry, so a value
/// with spaces or shell metacharacters stays one argument. `{name...}` splits
/// the value on whitespace into several arguments. A word that is exactly one
/// placeholder with an empty value is dropped.
pub fn expand(template: &str, vars: &HashMap<&str, String>) -> Result<Vec<String>, String> {
    let mut argv = Vec::new();
    for word in template.split_whitespace() {
        if let Some(name) = word.strip_prefix('{').and_then(|w| w.strip_suffix("...}")) {
            let v = vars.get(name).ok_or_else(|| format!("unknown placeholder {{{name}...}}"))?;
            argv.extend(v.split_whitespace().map(String::from));
            continue;
        }
        let mut out = String::new();
        let mut rest = word;
        while let Some(start) = rest.find('{') {
            let Some(len) = rest[start..].find('}') else { break };
            let name = &rest[start + 1..start + len];
            let v = vars.get(name).ok_or_else(|| format!("unknown placeholder {{{name}}}"))?;
            out.push_str(&rest[..start]);
            out.push_str(v);
            rest = &rest[start + len + 1..];
        }
        out.push_str(rest);
        let whole = word.starts_with('{') && word.ends_with('}');
        if !(out.is_empty() && whole) {
            argv.push(out);
        }
    }
    Ok(argv)
}

#[cfg(test)]
mod tests {
    use super::expand;
    use std::collections::HashMap;

    fn vars(pairs: &[(&'static str, &str)]) -> HashMap<&'static str, String> {
        pairs.iter().map(|(k, v)| (*k, v.to_string())).collect()
    }

    #[test]
    fn fills_placeholders_one_argv_entry_each() {
        let v = vars(&[("mailbox", "INBOX"), ("id", "42")]);
        assert_eq!(
            expand("himalaya message read -m {mailbox} {id}", &v).unwrap(),
            ["himalaya", "message", "read", "-m", "INBOX", "42"]
        );
    }

    #[test]
    fn hostile_values_stay_one_argument() {
        let v = vars(&[("query", "a b; rm -rf ~ $(reboot) {id}"), ("id", "1")]);
        assert_eq!(
            expand("cli search {query}", &v).unwrap(),
            ["cli", "search", "a b; rm -rf ~ $(reboot) {id}"]
        );
    }

    #[test]
    fn embedded_and_split_placeholders() {
        let v = vars(&[("page", "2"), ("query", "from ada")]);
        assert_eq!(expand("cli --page={page} {query...}", &v).unwrap(), ["cli", "--page=2", "from", "ada"]);
    }

    #[test]
    fn empty_whole_word_placeholder_is_dropped() {
        let v = vars(&[("mailbox", "")]);
        assert_eq!(expand("cli {mailbox} x", &v).unwrap(), ["cli", "x"]);
    }

    #[test]
    fn unknown_placeholder_is_an_error() {
        assert_eq!(expand("cli {nope}", &vars(&[])).unwrap_err(), "unknown placeholder {nope}");
    }
}
