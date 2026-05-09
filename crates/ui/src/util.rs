use std::net::SocketAddr;

pub(crate) fn default_bind() -> SocketAddr {
    "127.0.0.1:4000"
        .parse()
        .expect("default graphium-ui bind must be a valid socket address")
}

pub(crate) fn slugify(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut prev_dash = false;
    for ch in name.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            out.push(lower);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

pub(crate) fn normalize_symbol(value: &str) -> String {
    let cleaned = value.replace(' ', "").replace('\n', "");
    cleaned.rsplit("::").next().unwrap_or(&cleaned).to_string()
}

pub(crate) fn escape_label(value: &str) -> String {
    value.replace('"', "'").replace('\n', " ")
}

pub(crate) fn escape_pre(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

pub(crate) fn next_id(counter: &mut usize) -> String {
    let id = format!("n{counter}");
    *counter += 1;
    id
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ArtifactAccess {
    Owned,
    Borrowed,
    Taken,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ParsedArtifact<'a> {
    pub(crate) name: &'a str,
    pub(crate) access: ArtifactAccess,
    pub(crate) lifetime: Option<&'a str>,
    pub(crate) mutable: bool,
}

pub(crate) fn parse_artifact(value: &str) -> ParsedArtifact<'_> {
    let mut rest = value.trim();

    let access = if let Some(after) = rest.strip_prefix('&') {
        rest = after.trim_start();
        ArtifactAccess::Borrowed
    } else if let Some(after) = rest.strip_prefix('*') {
        rest = after.trim_start();
        ArtifactAccess::Taken
    } else {
        ArtifactAccess::Owned
    };

    let mut mutable = false;
    if let Some(after) = rest.strip_prefix("mut ") {
        mutable = true;
        rest = after.trim_start();
    }

    let mut lifetime: Option<&str> = None;
    if rest.starts_with('\'') {
        let end = rest
            .find(char::is_whitespace)
            .unwrap_or_else(|| rest.len());
        lifetime = Some(&rest[..end]);
        rest = rest[end..].trim_start();
    }

    if let Some(after) = rest.strip_prefix("mut ") {
        mutable = true;
        rest = after.trim_start();
    }

    let name = rest.trim_matches(|ch: char| matches!(ch, '(' | ')' | ','));
    ParsedArtifact {
        name,
        access,
        lifetime,
        mutable,
    }
}
