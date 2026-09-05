//! Gemini embedding chokepoint and retrieval-format helpers.
//!
//! Mirrors `super::client::post_gemini` (which targets `:generateContent`) for
//! the `:embedContent` endpoint. `embedContent` has a different response shape
//! and (in Gemini's billing) only consumes prompt tokens, so it gets its own
//! private chokepoint rather than reusing `post_gemini`.
//!
//! Public re-exports are surfaced via `super::mod` with `pub use embedding::*`.

use serde::Deserialize;
use serde_json::json;

pub const EMBEDDING_MODEL: &str = "gemini-embedding-2";
pub const EMBEDDING_OUTPUT_DIMENSIONALITY: usize = 3072;

#[derive(Debug, Deserialize)]
struct EmbedContentResponse {
    embedding: Option<EmbedContentValues>,
}

#[derive(Debug, Deserialize)]
struct EmbedContentValues {
    values: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct EmbeddingVector {
    pub model: String,
    pub values: Vec<f32>,
}

pub fn format_retrieval_document(text: &str) -> String {
    format!("title: none | text: {}", text.trim())
}

pub fn format_retrieval_query(query: &str) -> String {
    format!("task: search result | query: {}", query.trim())
}

pub async fn embed_retrieval_document(
    pool: Option<&sqlx::SqlitePool>,
    api_key: &str,
    text: &str,
) -> Result<EmbeddingVector, String> {
    if text.trim().is_empty() {
        return Err("cannot embed empty text".to_string());
    }
    post_gemini_embedding(pool, api_key, &format_retrieval_document(text)).await
}

pub async fn embed_retrieval_query(
    pool: Option<&sqlx::SqlitePool>,
    api_key: &str,
    query: &str,
) -> Result<EmbeddingVector, String> {
    if query.trim().is_empty() {
        return Err("cannot embed empty text".to_string());
    }
    post_gemini_embedding(pool, api_key, &format_retrieval_query(query)).await
}

/// Embedding chokepoint. Mirrors `post_gemini` for the `:generateContent`
/// endpoint but talks to `:embedContent`, which returns a different schema
/// and (in Gemini's billing) only consumes prompt tokens.
async fn post_gemini_embedding(
    pool: Option<&sqlx::SqlitePool>,
    api_key: &str,
    text: &str,
) -> Result<EmbeddingVector, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("cannot embed empty text".to_string());
    }
    let started = std::time::Instant::now();
    let breaker_key = format!("embedding:{EMBEDDING_MODEL}");
    if let Err(err) = crate::rate_limit::app_limiter().check_circuit(&breaker_key) {
        let message = err.to_string();
        if let Some(pool) = pool {
            crate::gemini_usage::record(
                pool,
                "embedding",
                EMBEDDING_MODEL,
                &crate::gemini_usage::UsageMetadata::default(),
                0,
                Some(&message),
            )
            .await;
        }
        return Err(message);
    }
    if let Some(pool) = pool {
        if let Some(block_msg) = crate::app_config::block_message_if_budget_exceeded(pool).await {
            crate::gemini_usage::record(
                pool,
                "embedding",
                EMBEDDING_MODEL,
                &crate::gemini_usage::UsageMetadata::default(),
                0,
                Some(&block_msg),
            )
            .await;
            return Err(block_msg);
        }
    }

    let body = json!({
        "model": format!("models/{EMBEDDING_MODEL}"),
        "content": { "parts": [{ "text": trimmed }] },
        "output_dimensionality": EMBEDDING_OUTPUT_DIMENSIONALITY
    });

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{EMBEDDING_MODEL}:embedContent"
    );

    let response = match crate::runtime::http_client()
        .post(url)
        .query(&[("key", api_key)])
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(error) => {
            let message = format!("Gemini embedding request failed: {error}");
            crate::rate_limit::app_limiter().note_failure(&breaker_key);
            if let Some(pool) = pool {
                crate::gemini_usage::record(
                    pool,
                    "embedding",
                    EMBEDDING_MODEL,
                    &crate::gemini_usage::UsageMetadata::default(),
                    started.elapsed().as_millis() as i64,
                    Some(&message),
                )
                .await;
            }
            return Err(message);
        }
    };

    if !response.status().is_success() {
        let status = response.status();
        let body_text = response.text().await.unwrap_or_default();
        let message = format!("Gemini embedding request failed with {status}: {body_text}");
        crate::rate_limit::app_limiter().note_failure(&breaker_key);
        if let Some(pool) = pool {
            crate::gemini_usage::record(
                pool,
                "embedding",
                EMBEDDING_MODEL,
                &crate::gemini_usage::UsageMetadata::default(),
                started.elapsed().as_millis() as i64,
                Some(&message),
            )
            .await;
        }
        return Err(message);
    }

    // Parse twice: once typed for validation, once raw to extract usageMetadata.
    let raw_json: serde_json::Value = match response.json().await {
        Ok(json) => json,
        Err(error) => {
            let message = format!("Gemini embedding response was not valid JSON: {error}");
            crate::rate_limit::app_limiter().note_failure(&breaker_key);
            if let Some(pool) = pool {
                crate::gemini_usage::record(
                    pool,
                    "embedding",
                    EMBEDDING_MODEL,
                    &crate::gemini_usage::UsageMetadata::default(),
                    started.elapsed().as_millis() as i64,
                    Some(&message),
                )
                .await;
            }
            return Err(message);
        }
    };
    let parsed: EmbedContentResponse = serde_json::from_value(raw_json.clone())
        .map_err(|error| format!("Gemini embedding response missed expected fields: {error}"))?;

    let values = parsed
        .embedding
        .map(|e| e.values)
        .ok_or_else(|| "Gemini embedding response had no embedding".to_string())?;

    if values.is_empty() {
        return Err("Gemini embedding returned an empty vector".to_string());
    }
    if values.len() != EMBEDDING_OUTPUT_DIMENSIONALITY {
        return Err(format!(
            "Gemini embedding returned {} dimensions, expected {}",
            values.len(),
            EMBEDDING_OUTPUT_DIMENSIONALITY
        ));
    }

    crate::rate_limit::app_limiter().note_success(&breaker_key);

    if let Some(pool) = pool {
        // embedContent reports tokens under `usageMetadata` similarly to
        // generateContent; if absent, fall back to a 0-token record so we
        // still see the call count and latency.
        let usage = crate::gemini_usage::parse_from_response(&raw_json);
        crate::gemini_usage::record(
            pool,
            "embedding",
            EMBEDDING_MODEL,
            &usage,
            started.elapsed().as_millis() as i64,
            None,
        )
        .await;
    }

    Ok(EmbeddingVector {
        model: EMBEDDING_MODEL.to_string(),
        values,
    })
}

#[cfg(test)]
mod embedding_tests {
    use super::*;

    #[test]
    fn formats_retrieval_document_for_embedding_2() {
        assert_eq!(
            format_retrieval_document("  Meeting notes about launch  "),
            "title: none | text: Meeting notes about launch"
        );
    }

    #[test]
    fn formats_retrieval_query_for_embedding_2() {
        assert_eq!(
            format_retrieval_query("  launch blockers  "),
            "task: search result | query: launch blockers"
        );
    }
}
