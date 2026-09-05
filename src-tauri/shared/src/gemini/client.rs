//! Gemini HTTP client chokepoint.
//!
//! Every `generateContent` request to the Gemini API goes through `post_gemini`.
//! It applies the circuit breaker (`rate_limit::app_limiter`), the configured
//! budget gate (`app_config::block_message_if_budget_exceeded`), and writes a
//! usage row (`gemini_usage::record`) on both success and failure paths so the
//! Settings panel can show per-feature cost.
//!
//! `post_gemini_external` is the public re-export consumed by sibling crate
//! modules (gmail, capture_promotion, eval, …). Sibling submodules under
//! `gemini::*` call `post_gemini` directly via `super::client::post_gemini`.

fn gemini_endpoint(model: &str) -> String {
    format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent")
}

/// External-facing alias for `post_gemini` so other crate modules (gmail, etc.)
/// route their Gemini calls through the same cost-tracking chokepoint.
pub async fn post_gemini_external(
    pool: Option<&sqlx::SqlitePool>,
    feature: &str,
    model: &str,
    api_key: &str,
    body: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    post_gemini(pool, feature, model, api_key, body).await
}

/// Single chokepoint for Gemini `generateContent` calls. Captures latency,
/// usage metadata, and optionally writes one `gemini_usage_log` row so the
/// Settings panel can show per-feature cost.
pub(super) async fn post_gemini(
    pool: Option<&sqlx::SqlitePool>,
    feature: &str,
    model: &str,
    api_key: &str,
    body: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let started = std::time::Instant::now();
    let breaker_key = format!("{feature}:{model}");

    if let Err(err) = crate::rate_limit::app_limiter().check_circuit(&breaker_key) {
        let message = err.to_string();
        if let Some(pool) = pool {
            crate::gemini_usage::record(
                pool,
                feature,
                model,
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
            // Budget enforcement: bail before sending. We deliberately do NOT
            // call note_failure here — this is a user-configured limit, not an
            // upstream/transport failure, so we shouldn't trip the breaker.
            crate::gemini_usage::record(
                pool,
                feature,
                model,
                &crate::gemini_usage::UsageMetadata::default(),
                0,
                Some(&block_msg),
            )
            .await;
            return Err(block_msg);
        }
    }

    let result = crate::runtime::http_client()
        .post(gemini_endpoint(model))
        .query(&[("key", api_key)])
        .json(body)
        .send()
        .await;

    let response = match result {
        Ok(response) => response,
        Err(error) => {
            let message = format!("Gemini request failed: {error}");
            crate::rate_limit::app_limiter().note_failure(&breaker_key);
            if let Some(pool) = pool {
                crate::gemini_usage::record(
                    pool,
                    feature,
                    model,
                    &crate::gemini_usage::UsageMetadata::default(),
                    started.elapsed().as_millis() as i64,
                    Some(&message),
                )
                .await;
            }
            return Err(message);
        }
    };

    let status = response.status();
    if !status.is_success() {
        let body_text = response.text().await.unwrap_or_default();
        let message = format!("Gemini request failed with {status}: {body_text}");
        crate::rate_limit::app_limiter().note_failure(&breaker_key);
        if let Some(pool) = pool {
            crate::gemini_usage::record(
                pool,
                feature,
                model,
                &crate::gemini_usage::UsageMetadata::default(),
                started.elapsed().as_millis() as i64,
                Some(&message),
            )
            .await;
        }
        return Err(message);
    }

    let json: serde_json::Value = match response.json().await {
        Ok(json) => json,
        Err(error) => {
            let message = format!("Gemini response was not valid JSON: {error}");
            crate::rate_limit::app_limiter().note_failure(&breaker_key);
            if let Some(pool) = pool {
                crate::gemini_usage::record(
                    pool,
                    feature,
                    model,
                    &crate::gemini_usage::UsageMetadata::default(),
                    started.elapsed().as_millis() as i64,
                    Some(&message),
                )
                .await;
            }
            return Err(message);
        }
    };

    crate::rate_limit::app_limiter().note_success(&breaker_key);

    if let Some(pool) = pool {
        let usage = crate::gemini_usage::parse_from_response(&json);
        crate::gemini_usage::record(
            pool,
            feature,
            model,
            &usage,
            started.elapsed().as_millis() as i64,
            None,
        )
        .await;
    }

    Ok(json)
}
