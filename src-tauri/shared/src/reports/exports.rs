use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::json;
use sqlx::SqlitePool;
use zip::write::SimpleFileOptions;

use crate::db::sql_error;
use crate::models::{ExportReportInput, ReportExport};

use super::{provenance, service};

/// Matches `[S1]`, `[S12]`, etc. — Gemini citation markers embedded in section drafts.
static CITATION_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\[S(\d+)\]").unwrap());

pub async fn list_report_exports(
    pool: &SqlitePool,
    report_run_id: &str,
) -> Result<Vec<ReportExport>, String> {
    sqlx::query_as::<_, ReportExport>(
        r#"SELECT id, report_run_id, format, export_path, artifact_url, created_at
           FROM report_exports WHERE report_run_id = ? ORDER BY created_at DESC"#,
    )
    .bind(report_run_id)
    .fetch_all(pool)
    .await
    .map_err(sql_error)
}

pub async fn export_report(
    pool: &SqlitePool,
    app_support_dir: &Path,
    input: ExportReportInput,
) -> Result<ReportExport, String> {
    let report = service::get_report_run(pool, &input.report_run_id).await?;
    if report.status != "approved" {
        return Err("Approve the report before exporting it.".to_string());
    }
    // Strip raw [S#] citation markers before writing any export format.
    // These markers are kept in the DB for UI rendering (styled as badges)
    // but are meaningless in exported documents without a footnote section.
    let export_markdown = strip_citation_markers(&report.draft_markdown);
    let format = input.format.as_str();
    let (export_path, artifact_url) = match format {
        "markdown" | "docx" | "pdf" => {
            let directory = input
                .destination_dir
                .as_ref()
                .map(PathBuf::from)
                .ok_or_else(|| "Choose an export directory.".to_string())?;
            std::fs::create_dir_all(&directory)
                .map_err(|error| format!("Could not create export directory: {error}"))?;
            let path = directory.join(format!("{}.{}", slug(&report.title), extension(format)));
            match format {
                "markdown" => std::fs::write(&path, &export_markdown)
                    .map_err(|error| format!("Could not write Markdown export: {error}"))?,
                "docx" => write_docx(&path, &report.title, &export_markdown)?,
                "pdf" => write_pdf(&path, &report.title, &export_markdown)?,
                _ => unreachable!(),
            }
            (Some(path.display().to_string()), None)
        }
        "google_docs" => {
            if !input.external_write_confirmed {
                return Err("Google Docs export requires explicit confirmation.".to_string());
            }
            let existing_url: Option<String> = sqlx::query_scalar(
                r#"SELECT artifact_url FROM report_exports
                   WHERE report_run_id = ? AND format = 'google_docs' AND artifact_url IS NOT NULL
                   ORDER BY created_at DESC LIMIT 1"#,
            )
            .bind(&report.id)
            .fetch_optional(pool)
            .await
            .map_err(sql_error)?
            .flatten();
            let url =
                if let Some(document_id) = existing_url.as_deref().and_then(google_document_id) {
                    crate::google_docs::replace_document_text(
                        app_support_dir,
                        document_id,
                        &export_markdown,
                    )
                    .await?
                } else {
                    crate::google_docs::create_document(
                        app_support_dir,
                        &report.title,
                        &export_markdown,
                    )
                    .await?
                    .1
                };
            (None, Some(url))
        }
        _ => return Err("Unsupported report export format.".to_string()),
    };
    let export = ReportExport {
        id: format!("report_export:{}", ulid::Ulid::new()),
        report_run_id: input.report_run_id.clone(),
        format: input.format,
        export_path,
        artifact_url,
        created_at: crate::repo::now_utc(),
    };
    sqlx::query(
        r#"INSERT INTO report_exports
           (id, report_run_id, format, export_path, artifact_url, created_at)
           VALUES (?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&export.id)
    .bind(&export.report_run_id)
    .bind(&export.format)
    .bind(&export.export_path)
    .bind(&export.artifact_url)
    .bind(&export.created_at)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    provenance::record_event(
        pool,
        &export.report_run_id,
        "exported",
        json!({ "format": export.format, "path": export.export_path, "artifact_url": export.artifact_url }),
    )
    .await?;
    Ok(export)
}

/// Remove `[S1]`, `[S2]` … citation markers from markdown before export.
/// The markers are preserved in the DB for UI rendering (styled as badges)
/// but produce confusing noise in exported documents without a footnote list.
fn strip_citation_markers(markdown: &str) -> String {
    CITATION_RE.replace_all(markdown, "").to_string()
}

fn slug(value: &str) -> String {
    let normalized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let mut output = normalized
        .split('-')
        .filter(|chunk| !chunk.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if output.is_empty() {
        output = "trace-report".to_string();
    }
    output
}

fn extension(format: &str) -> &str {
    if format == "markdown" {
        "md"
    } else {
        format
    }
}

fn google_document_id(url: &str) -> Option<&str> {
    url.split("/document/d/").nth(1)?.split('/').next()
}

fn write_docx(path: &Path, title: &str, markdown: &str) -> Result<(), String> {
    let file = File::create(path).map_err(|error| format!("Could not create DOCX: {error}"))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default();
    zip.start_file("[Content_Types].xml", options)
        .map_err(|error| format!("Could not build DOCX: {error}"))?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#)
        .map_err(|error| format!("Could not build DOCX: {error}"))?;
    zip.add_directory("_rels/", options)
        .map_err(|error| format!("Could not build DOCX: {error}"))?;
    zip.start_file("_rels/.rels", options)
        .map_err(|error| format!("Could not build DOCX: {error}"))?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#)
        .map_err(|error| format!("Could not build DOCX: {error}"))?;
    zip.add_directory("word/", options)
        .map_err(|error| format!("Could not build DOCX: {error}"))?;
    zip.start_file("word/document.xml", options)
        .map_err(|error| format!("Could not build DOCX: {error}"))?;
    let paragraphs = std::iter::once(format!("# {title}"))
        .chain(markdown.lines().map(ToString::to_string))
        .map(|line| {
            format!(
                "<w:p><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
                xml_escape(&line)
            )
        })
        .collect::<String>();
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{paragraphs}</w:body></w:document>"#
    );
    zip.write_all(document.as_bytes())
        .map_err(|error| format!("Could not build DOCX: {error}"))?;
    zip.finish()
        .map_err(|error| format!("Could not finish DOCX: {error}"))?;
    Ok(())
}

fn write_pdf(path: &Path, title: &str, markdown: &str) -> Result<(), String> {
    let mut lines = vec![title.to_string()];
    lines.extend(
        markdown
            .lines()
            .map(|line| line.trim_matches('#').trim().to_string())
            .filter(|line| !line.is_empty())
            .take(48),
    );
    let stream = lines
        .iter()
        .map(|line| format!("({}) Tj 0 -14 Td", pdf_escape(&truncate(line, 95))))
        .collect::<Vec<_>>()
        .join("\n");
    let content = format!("BT /F1 11 Tf 50 780 Td\n{stream}\nET");
    let objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".to_string(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
        format!("<< /Length {} >>\nstream\n{}\nendstream", content.len(), content),
    ];
    let mut pdf = "%PDF-1.4\n".to_string();
    let mut offsets = vec![0usize];
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.push_str(&format!("{} 0 obj\n{}\nendobj\n", index + 1, object));
    }
    let xref = pdf.len();
    pdf.push_str(&format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len()));
    for offset in offsets.into_iter().skip(1) {
        pdf.push_str(&format!("{offset:010} 00000 n \n"));
    }
    pdf.push_str(&format!(
        "trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n"
    ));
    std::fs::write(path, pdf).map_err(|error| format!("Could not create PDF: {error}"))
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn pdf_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

fn truncate(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_stable_export_slugs() {
        assert_eq!(slug("Q3 Report: Core Work"), "q3-report-core-work");
    }

    #[test]
    fn extracts_google_document_id_for_update_exports() {
        assert_eq!(
            google_document_id("https://docs.google.com/document/d/doc-123/edit"),
            Some("doc-123")
        );
    }
}
