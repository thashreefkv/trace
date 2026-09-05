use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DocsDocument {
    pub document_id: String,
    pub title: String,
    pub body: DocsBody,
    #[serde(default)]
    pub revision_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DocsBody {
    #[serde(default)]
    pub content: Vec<StructuralElement>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct StructuralElement {
    pub paragraph: Option<Paragraph>,
    pub table: Option<DocsTable>,
    pub section_break: Option<Value>,
}

impl Default for StructuralElement {
    fn default() -> Self {
        Self {
            paragraph: None,
            table: None,
            section_break: None,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Paragraph {
    pub elements: Vec<ParagraphElement>,
    pub paragraph_style: Option<ParagraphStyle>,
    pub bullet: Option<Bullet>,
}

impl Default for Paragraph {
    fn default() -> Self {
        Self {
            elements: vec![],
            paragraph_style: None,
            bullet: None,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ParagraphStyle {
    /// "NORMAL_TEXT" | "HEADING_1" .. "HEADING_6" | "TITLE"
    pub named_style_type: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Bullet {
    pub list_id: String,
    pub nesting_level: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ParagraphElement {
    pub text_run: Option<TextRun>,
    pub inline_object_element: Option<Value>,
    pub start_index: Option<i32>,
    pub end_index: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct TextRun {
    pub content: String,
    pub text_style: Option<TextStyle>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct TextStyle {
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strikethrough: Option<bool>,
    pub link: Option<DocsLink>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(default)]
pub struct DocsLink {
    pub url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct DocsTable {
    /// Google returns an integer row count here, not the rows themselves.
    /// The actual rows are in `table_rows`. Kept as a tolerant `Value` so a
    /// schema drift in either direction doesn't crash the whole doc parse.
    pub rows: Option<Value>,
    pub columns: Option<Value>,
    pub table_rows: Option<Vec<Value>>,
}

// ── API calls ─────────────────────────────────────────────────────────────────

pub async fn get_document(dir: &Path, document_id: &str) -> Result<DocsDocument, String> {
    let token = crate::google_drive::get_valid_access_token(dir).await?;
    let resp = reqwest::Client::new()
        .get(format!(
            "https://docs.googleapis.com/v1/documents/{document_id}"
        ))
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Docs API {status} {body}"));
    }
    serde_json::from_str::<DocsDocument>(&body).map_err(|e| {
        let preview: String = body.chars().take(400).collect();
        format!("Docs API decode error: {e}\n--- response body preview ---\n{preview}")
    })
}

/// Apply a list of batchUpdate requests.
/// Each request is a raw JSON object like:
///   {"insertText": {"location": {"index": 5}, "text": "hello"}}
///   {"updateTextStyle": {...}}
///   {"deleteContentRange": {"range": {"startIndex": 5, "endIndex": 10}}}
pub async fn batch_update(
    dir: &Path,
    document_id: &str,
    requests: Vec<Value>,
) -> Result<(), String> {
    if requests.is_empty() {
        return Ok(());
    }
    let token = crate::google_drive::get_valid_access_token(dir).await?;
    let body = json!({ "requests": requests });
    let resp = reqwest::Client::new()
        .post(format!(
            "https://docs.googleapis.com/v1/documents/{document_id}:batchUpdate"
        ))
        .bearer_auth(&token)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!(
            "batchUpdate {} {}",
            resp.status(),
            resp.text().await.unwrap_or_default()
        ));
    }
    Ok(())
}

/// Create a Google Doc and insert canonical Markdown as its initial text.
/// Callers must obtain user confirmation before invoking this external write.
pub async fn create_document(
    dir: &Path,
    title: &str,
    body_text: &str,
) -> Result<(String, String), String> {
    let token = crate::google_drive::get_valid_access_token(dir).await?;
    let resp = reqwest::Client::new()
        .post("https://docs.googleapis.com/v1/documents")
        .bearer_auth(&token)
        .json(&json!({ "title": title }))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = resp.status();
    let raw = resp.text().await.map_err(|error| error.to_string())?;
    if !status.is_success() {
        return Err(format!("Docs create {status} {raw}"));
    }
    let document_id = serde_json::from_str::<Value>(&raw)
        .map_err(|error| format!("Docs create decode error: {error}"))?
        .get("documentId")
        .and_then(Value::as_str)
        .ok_or_else(|| "Docs create response had no documentId".to_string())?
        .to_string();
    if !body_text.is_empty() {
        batch_update(
            dir,
            &document_id,
            vec![json!({ "insertText": { "location": { "index": 1 }, "text": body_text } })],
        )
        .await?;
    }
    Ok((
        document_id.clone(),
        format!("https://docs.google.com/document/d/{document_id}/edit"),
    ))
}

pub async fn replace_document_text(
    dir: &Path,
    document_id: &str,
    body_text: &str,
) -> Result<String, String> {
    let document = get_document(dir, document_id).await?;
    let end_index = document
        .body
        .content
        .iter()
        .flat_map(|element| element.paragraph.iter())
        .flat_map(|paragraph| paragraph.elements.iter())
        .filter_map(|element| element.end_index)
        .max()
        .unwrap_or(1);
    let mut requests = Vec::new();
    if end_index > 1 {
        requests.push(json!({
            "deleteContentRange": { "range": { "startIndex": 1, "endIndex": end_index - 1 } }
        }));
    }
    if !body_text.is_empty() {
        requests.push(json!({ "insertText": { "location": { "index": 1 }, "text": body_text } }));
    }
    batch_update(dir, document_id, requests).await?;
    Ok(format!(
        "https://docs.google.com/document/d/{document_id}/edit"
    ))
}

// ── Slides ────────────────────────────────────────────────────────────────────

pub async fn get_presentation(dir: &Path, presentation_id: &str) -> Result<Value, String> {
    let token = crate::google_drive::get_valid_access_token(dir).await?;
    let resp = reqwest::Client::new()
        .get(format!(
            "https://slides.googleapis.com/v1/presentations/{presentation_id}"
        ))
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Slides API {status} {body}"));
    }
    serde_json::from_str::<Value>(&body).map_err(|e| {
        let preview: String = body.chars().take(400).collect();
        format!("Slides API decode error: {e}\n--- response body preview ---\n{preview}")
    })
}

pub async fn get_slide_thumbnail(
    dir: &Path,
    presentation_id: &str,
    page_object_id: &str,
) -> Result<String, String> {
    let token = crate::google_drive::get_valid_access_token(dir).await?;
    let resp = reqwest::Client::new()
        .get(format!(
            "https://slides.googleapis.com/v1/presentations/{presentation_id}/pages/{page_object_id}/thumbnail"
        ))
        .query(&[("thumbnailProperties.thumbnailSize", "LARGE")])
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!(
            "Slides thumbnail {} {}",
            resp.status(),
            resp.text().await.unwrap_or_default()
        ));
    }
    let j: Value = resp.json().await.map_err(|e| e.to_string())?;
    j["contentUrl"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "no contentUrl in thumbnail response".to_string())
}

// ── Sheets ────────────────────────────────────────────────────────────────────

/// Returns the raw Spreadsheet resource from sheets.googleapis.com v4 with
/// grid data included. The frontend renders this directly — keeping the
/// type as `Value` avoids a full Rust mirror of the (deeply nested) Sheets
/// schema.
pub async fn get_spreadsheet(dir: &Path, spreadsheet_id: &str) -> Result<Value, String> {
    let token = crate::google_drive::get_valid_access_token(dir).await?;
    let resp = reqwest::Client::new()
        .get(format!(
            "https://sheets.googleapis.com/v4/spreadsheets/{spreadsheet_id}"
        ))
        .query(&[
            ("includeGridData", "true"),
            // Limit to the first sheet's first 200 rows × 26 cols to keep the
            // payload manageable for an in-app preview. Users can hit "Open in
            // browser" for full editing.
            ("ranges", "A1:Z200"),
        ])
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Sheets API {status} {body}"));
    }
    serde_json::from_str::<Value>(&body).map_err(|e| {
        let preview: String = body.chars().take(400).collect();
        format!("Sheets API decode error: {e}\n--- response body preview ---\n{preview}")
    })
}
