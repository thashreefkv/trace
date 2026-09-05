use std::collections::BTreeSet;

use chrono::{SecondsFormat, TimeZone, Utc};
use serde::Serialize;

use super::{EmailAddress, Header, ParsedMessage};

pub fn message_preview(parsed: &ParsedMessage) -> String {
    let body = if parsed.plain_body.trim().is_empty() {
        strip_html(&parsed.html_body)
    } else {
        parsed.plain_body.clone()
    };
    truncate_text(body.trim(), 500)
}

pub fn truncate_text(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let prefix = value
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    format!("{prefix}...")
}

pub fn header_value(headers: &[Header], name: &str) -> Option<String> {
    headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case(name))
        .map(|header| header.value.clone())
}

pub fn parse_address_header(value: &str) -> Vec<EmailAddress> {
    value
        .split(',')
        .map(parse_single_address)
        .filter(|address| !address.email.is_empty())
        .collect()
}

pub fn parse_single_address(value: &str) -> EmailAddress {
    let value = value.trim();
    if let (Some(start), Some(end)) = (value.rfind('<'), value.rfind('>')) {
        let name = value[..start].trim().trim_matches('"').to_string();
        let email = normalize_email(&value[start + 1..end]);
        return EmailAddress {
            name: if name.is_empty() { email.clone() } else { name },
            email,
        };
    }
    let email = normalize_email(value.trim_matches('"'));
    EmailAddress {
        name: email.clone(),
        email,
    }
}

pub fn normalize_email(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_lowercase()
}

pub fn parse_addresses_json(value: &str) -> Vec<EmailAddress> {
    serde_json::from_str::<Vec<EmailAddress>>(value).unwrap_or_default()
}

pub fn parse_string_vec(value: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(value).unwrap_or_default()
}

pub fn to_json_string<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "[]".to_string())
}

pub fn strip_html(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut in_tag = false;
    let mut in_entity = false;
    let mut entity = String::new();
    for ch in value.chars() {
        match ch {
            '<' => {
                in_tag = true;
                output.push(' ');
            }
            '>' => in_tag = false,
            '&' if !in_tag => {
                in_entity = true;
                entity.clear();
            }
            ';' if in_entity => {
                in_entity = false;
                output.push_str(match entity.as_str() {
                    "amp" => "&",
                    "lt" => "<",
                    "gt" => ">",
                    "quot" => "\"",
                    "nbsp" => " ",
                    _ => "",
                });
            }
            _ if in_tag => {}
            _ if in_entity => entity.push(ch),
            _ => output.push(ch),
        }
    }
    output
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

pub fn extract_urls(value: &str) -> Vec<String> {
    let mut urls = BTreeSet::new();
    for token in value.split_whitespace() {
        let cleaned = token
            .trim_matches(|ch: char| {
                matches!(
                    ch,
                    '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | ',' | ';'
                )
            })
            .trim_end_matches('.');
        if cleaned.starts_with("https://") || cleaned.starts_with("http://") {
            urls.insert(cleaned.to_string());
        }
    }
    urls.into_iter().collect()
}

pub fn classify_url(url: &str) -> &'static str {
    let lower = url.to_lowercase();
    if lower.contains("figma.com") {
        "figma"
    } else if lower.contains("docs.google.com/document") {
        "doc"
    } else if lower.contains("docs.google.com/presentation") {
        "slides"
    } else if lower.contains("docs.google.com/spreadsheets") {
        "sheet"
    } else if lower.contains("notion.so") {
        "notion"
    } else if lower.contains("github.com") {
        "github"
    } else {
        "url"
    }
}

pub fn is_artifact_url(url: &str) -> bool {
    classify_url(url) != "url"
}

pub fn email_domain(email: &str) -> Option<String> {
    let domain = email
        .rsplit_once('@')
        .map(|(_, domain)| domain.trim().trim_matches('>').to_lowercase())?;
    if domain.is_empty()
        || matches!(
            domain.as_str(),
            "gmail.com"
                | "googlemail.com"
                | "yahoo.com"
                | "outlook.com"
                | "hotmail.com"
                | "icloud.com"
                | "me.com"
                | "aol.com"
                | "proton.me"
                | "protonmail.com"
        )
    {
        return None;
    }
    Some(domain)
}

pub fn format_ts(ts: Option<i64>) -> Option<String> {
    ts.and_then(|ts| Utc.timestamp_opt(ts, 0).single())
        .map(|dt| dt.to_rfc3339_opts(SecondsFormat::Millis, true))
}

pub fn now_utc() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub fn max_history(current: Option<String>, candidate: &str) -> Option<String> {
    let candidate_num = candidate.parse::<u128>().ok()?;
    let current_num = current
        .as_deref()
        .and_then(|value| value.parse::<u128>().ok());
    if current_num.map_or(true, |current| candidate_num > current) {
        Some(candidate.to_string())
    } else {
        current
    }
}

