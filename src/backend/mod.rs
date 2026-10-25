//! The backend: any email CLI, described by command templates.

pub mod exec;
pub mod fields;
pub mod template;

use crate::config::BackendSpec;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Envelope {
    pub id: String,
    pub subject: String,
    pub from: String,
    pub date: String,
    pub seen: bool,
    pub flagged: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Flag {
    Seen,
    Flagged,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Draft {
    New,
    Reply,
    ReplyAll,
    Forward,
}

#[derive(Debug, Clone)]
pub struct Backend {
    spec: BackendSpec,
    page_size: u32,
}

impl Backend {
    pub const ACTIONS: [&'static str; 13] = [
        "mailboxes", "list", "search", "read", "flag", "unflag", "move", "delete", "send", "compose", "reply",
        "reply_all", "forward",
    ];

    pub fn new(spec: BackendSpec, page_size: u32) -> Self {
        Self { spec, page_size }
    }

    /// The template for an action, by its config key.
    pub fn template(&self, action: &str) -> Option<&str> {
        let s = &self.spec;
        match action {
            "mailboxes" => s.mailboxes.as_deref(),
            "list" => s.list.as_deref(),
            "search" => s.search.as_deref(),
            "read" => s.read.as_deref(),
            "flag" => s.flag.as_deref(),
            "unflag" => s.unflag.as_deref(),
            "move" => s.move_.as_deref(),
            "delete" => s.delete.as_deref(),
            "send" => s.send.as_deref(),
            "compose" => s.compose.as_deref(),
            "reply" => s.reply.as_deref(),
            "reply_all" => s.reply_all.as_deref(),
            "forward" => s.forward.as_deref(),
            _ => None,
        }
    }

    pub fn has(&self, action: &str) -> bool {
        self.template(action).is_some()
    }

    fn call(&self, action: &str, vars: &[(&str, &str)], stdin: Option<&str>) -> Result<String, String> {
        let tpl = self.template(action).ok_or_else(|| format!("no `{action}` template configured"))?;
        let mut map: HashMap<&str, String> = vars.iter().map(|(k, v)| (*k, v.to_string())).collect();
        map.insert("size", self.page_size.to_string());
        let argv = template::expand(tpl, &map).map_err(|e| format!("{action}: {e}"))?;
        exec::run(&argv, stdin.map(str::as_bytes)).map_err(|e| format!("{action}: {e}"))
    }

    fn json(&self, action: &str, vars: &[(&str, &str)]) -> Result<Value, String> {
        let out = self.call(action, vars, None)?;
        serde_json::from_str(&out).map_err(|e| format!("{action}: output is not JSON ({e})"))
    }

    fn items<'a>(action: &str, v: &'a Value, path: &str) -> Result<&'a Vec<Value>, String> {
        fields::get(v, path).and_then(Value::as_array).ok_or_else(|| format!("{action}: no array at `{path}`"))
    }

    pub fn mailboxes(&self) -> Result<Vec<String>, String> {
        let f = &self.spec.fields;
        let v = self.json("mailboxes", &[])?;
        Self::items("mailboxes", &v, &f.mailbox_items)?
            .iter()
            .map(|m| fields::text(m, &f.mailbox_name).ok_or_else(|| format!("mailboxes: item has no `{}`", f.mailbox_name)))
            .collect()
    }

    pub fn list(&self, mailbox: &str, page: u32) -> Result<Vec<Envelope>, String> {
        let v = self.json("list", &[("mailbox", mailbox), ("page", &page.to_string())])?;
        self.envelopes("list", &v)
    }

    pub fn search(&self, mailbox: &str, query: &str, page: u32) -> Result<Vec<Envelope>, String> {
        let v = self.json("search", &[("mailbox", mailbox), ("query", query), ("page", &page.to_string())])?;
        self.envelopes("search", &v)
    }

    fn envelopes(&self, action: &str, v: &Value) -> Result<Vec<Envelope>, String> {
        let f = &self.spec.fields;
        Self::items(action, v, &f.items)?
            .iter()
            .map(|item| {
                let id = fields::text(item, &f.id).ok_or_else(|| format!("{action}: item has no `{}`", f.id))?;
                let flags: Vec<String> = fields::get(item, &f.flags)
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| match x {
                                Value::String(s) => Some(s.clone()),
                                other => fields::text(other, &f.flag_name),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let has = |name: &str| !name.is_empty() && flags.iter().any(|x| x.eq_ignore_ascii_case(name));
                Ok(Envelope {
                    id,
                    subject: fields::text(item, &f.subject).unwrap_or_default(),
                    from: fields::text(item, &f.from).unwrap_or_default(),
                    date: fields::text(item, &f.date).unwrap_or_default(),
                    seen: has(&self.spec.seen_flag),
                    flagged: has(&self.spec.flagged_flag),
                })
            })
            .collect()
    }

    pub fn read(&self, mailbox: &str, id: &str) -> Result<String, String> {
        self.call("read", &[("mailbox", mailbox), ("id", id)], None)
    }

    pub fn set_flag(&self, mailbox: &str, id: &str, flag: Flag, on: bool) -> Result<(), String> {
        let name = match flag {
            Flag::Seen => &self.spec.seen_flag,
            Flag::Flagged => &self.spec.flagged_flag,
        };
        let action = if on { "flag" } else { "unflag" };
        self.call(action, &[("mailbox", mailbox), ("id", id), ("flag", name)], None).map(drop)
    }

    pub fn move_to(&self, mailbox: &str, id: &str, target: &str) -> Result<(), String> {
        self.call("move", &[("mailbox", mailbox), ("id", id), ("target", target)], None).map(drop)
    }

    pub fn delete(&self, mailbox: &str, id: &str) -> Result<(), String> {
        self.call("delete", &[("mailbox", mailbox), ("id", id)], None).map(drop)
    }

    /// Send a raw RFC 5322 message on the template's stdin.
    pub fn send(&self, raw: &str) -> Result<(), String> {
        self.call("send", &[], Some(raw)).map(drop)
    }

    /// Text to open in $EDITOR. A new message without a `compose` template
    /// starts from a blank header block.
    pub fn draft(&self, kind: Draft, mailbox: &str, id: &str) -> Result<String, String> {
        let action = match kind {
            Draft::New => "compose",
            Draft::Reply => "reply",
            Draft::ReplyAll => "reply_all",
            Draft::Forward => "forward",
        };
        if kind == Draft::New && !self.has("compose") {
            return Ok("To: \nCc: \nSubject: \n\n".into());
        }
        self.call(action, &[("mailbox", mailbox), ("id", id)], None)
    }
}
