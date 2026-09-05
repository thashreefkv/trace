//! Calendar agentic tool wrappers + Meet creation. From legacy.rs (13-std3).

use std::path::Path;

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use ulid::Ulid;

use super::{calendar_connected, create_event, delete_event, get_valid_access_token, update_event, GCAL_API};

#[derive(Serialize)]
struct GCalAttendeeInput {
    email: String,
}

#[derive(Serialize)]
struct GCalTimedEventBody<'a> {
    summary: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
    start: GCalTimedDate<'a>,
    end: GCalTimedDate<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attendees: Option<Vec<GCalAttendeeInput>>,
}

#[derive(Serialize)]
struct GCalTimedDate<'a> {
    #[serde(rename = "dateTime")]
    date_time: &'a str,
    #[serde(rename = "timeZone")]
    time_zone: &'a str,
}

/// Create a calendar event from the brain. Supports all-day (no times) or timed.
/// date: YYYY-MM-DD, start_time/end_time: HH:MM (24h), time_zone: IANA name e.g. "Asia/Kolkata"
pub async fn tool_create_calendar_event(
    pool: &SqlitePool,
    dir: &std::path::Path,
    title: &str,
    description: Option<&str>,
    date: &str,
    start_time: Option<&str>,
    end_time: Option<&str>,
    time_zone: Option<&str>,
    attendees: Option<Vec<String>>,
) -> serde_json::Value {
    if !calendar_connected(dir) {
        return serde_json::json!({ "error": "Google Calendar is not connected" });
    }
    let token = match get_valid_access_token(dir).await {
        Ok(t) => t,
        Err(e) => return serde_json::json!({ "error": e }),
    };
    // Default to IST — the user's timezone. Callers should always pass time_zone explicitly.
    let tz = time_zone.unwrap_or("Asia/Kolkata");

    let attendees_input: Option<Vec<GCalAttendeeInput>> = attendees.as_ref().map(|emails| {
        emails
            .iter()
            .map(|e| GCalAttendeeInput { email: e.clone() })
            .collect()
    });

    let gcal_event_id = match (start_time, end_time) {
        (Some(st), Some(et)) => {
            // Timed event
            let start_dt = format!("{date}T{st}:00");
            let end_dt = format!("{date}T{et}:00");
            let body = GCalTimedEventBody {
                summary: title,
                description,
                start: GCalTimedDate {
                    date_time: &start_dt,
                    time_zone: tz,
                },
                end: GCalTimedDate {
                    date_time: &end_dt,
                    time_zone: tz,
                },
                attendees: attendees_input,
            };
            let resp = match reqwest::Client::new()
                .post(format!("{GCAL_API}/calendars/primary/events"))
                .bearer_auth(&token)
                .query(&[("sendUpdates", "all")]) // send invite emails to attendees
                .json(&body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => return serde_json::json!({ "error": e.to_string() }),
            };
            if !resp.status().is_success() {
                return serde_json::json!({ "error": resp.text().await.unwrap_or_default() });
            }
            #[derive(Deserialize)]
            struct Created {
                id: String,
            }
            match resp.json::<Created>().await {
                Ok(c) => c.id,
                Err(e) => return serde_json::json!({ "error": e.to_string() }),
            }
        }
        _ => {
            // All-day event
            match create_event(&token, title, description, date, "manual", "brain").await {
                Ok(id) => id,
                Err(e) => return serde_json::json!({ "error": e }),
            }
        }
    };

    // Cache it locally (store times as local, not UTC)
    let now_iso = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let id = ulid::Ulid::new().to_string();
    let is_all_day = start_time.is_none();
    let start_dt_full = start_time.map(|st| format!("{date}T{st}:00"));
    let end_dt_full = end_time.map(|et| format!("{date}T{et}:00"));
    let _ = sqlx::query(
        "INSERT OR REPLACE INTO gcal_events
         (id, gcal_event_id, title, description, start_date, is_all_day, start_datetime, end_datetime, updated_at)
         VALUES (?,?,?,?,?,?,?,?,?)"
    )
    .bind(&id).bind(&gcal_event_id).bind(title).bind(description)
    .bind(date).bind(is_all_day).bind(&start_dt_full).bind(&end_dt_full).bind(&now_iso)
    .execute(pool).await;

    serde_json::json!({
        "success": true,
        "gcal_event_id": gcal_event_id,
        "title": title,
        "date": date,
        "start_time": start_time,
        "end_time": end_time,
        "time_zone": tz,
        "attendees_invited": attendees.unwrap_or_default(),
    })
}

/// Update a GCal event by its event ID. title/description/date/times are optional.
pub async fn tool_update_calendar_event(
    pool: &SqlitePool,
    dir: &std::path::Path,
    gcal_event_id: &str,
    title: Option<&str>,
    description: Option<&str>,
    date: Option<&str>,
    start_time: Option<&str>,
    end_time: Option<&str>,
) -> serde_json::Value {
    if !calendar_connected(dir) {
        return serde_json::json!({ "error": "Google Calendar is not connected" });
    }
    let token = match get_valid_access_token(dir).await {
        Ok(t) => t,
        Err(e) => return serde_json::json!({ "error": e }),
    };

    // Fetch current event from cache to fill in missing fields
    #[derive(sqlx::FromRow)]
    struct Row {
        title: String,
        description: Option<String>,
        start_date: String,
        start_datetime: Option<String>,
        end_datetime: Option<String>,
        is_all_day: bool,
    }
    let current = sqlx::query_as::<_, Row>(
        "SELECT title, description, start_date, start_datetime, end_datetime, is_all_day
         FROM gcal_events WHERE gcal_event_id = ?",
    )
    .bind(gcal_event_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    let use_title = title.unwrap_or(current.as_ref().map(|r| r.title.as_str()).unwrap_or(""));
    let use_desc = description.or(current.as_ref().and_then(|r| r.description.as_deref()));
    let use_date = date
        .or(current.as_ref().map(|r| r.start_date.as_str()))
        .unwrap_or("");

    if let (Some(st), Some(et)) = (start_time, end_time) {
        let start_dt = format!("{use_date}T{st}:00");
        let end_dt = format!("{use_date}T{et}:00");
        let body = GCalTimedEventBody {
            summary: use_title,
            description: use_desc,
            start: GCalTimedDate {
                date_time: &start_dt,
                time_zone: "Asia/Kolkata",
            },
            end: GCalTimedDate {
                date_time: &end_dt,
                time_zone: "Asia/Kolkata",
            },
            attendees: None,
        };
        let resp = match reqwest::Client::new()
            .put(format!(
                "{GCAL_API}/calendars/primary/events/{gcal_event_id}"
            ))
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => return serde_json::json!({ "error": e.to_string() }),
        };
        if !resp.status().is_success() {
            return serde_json::json!({ "error": resp.text().await.unwrap_or_default() });
        }
    } else {
        if let Err(e) = update_event(&token, gcal_event_id, use_title, use_desc, use_date).await {
            return serde_json::json!({ "error": e });
        }
    }

    // Update cache
    let now_iso = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let _ = sqlx::query(
        "UPDATE gcal_events SET title=?, description=?, start_date=?, updated_at=?
         WHERE gcal_event_id=?",
    )
    .bind(use_title)
    .bind(use_desc)
    .bind(use_date)
    .bind(&now_iso)
    .bind(gcal_event_id)
    .execute(pool)
    .await;

    serde_json::json!({ "success": true, "gcal_event_id": gcal_event_id, "title": use_title, "date": use_date })
}

// ── Schedule meeting with optional Google Meet link ───────────────────────────

#[derive(Serialize)]
struct GCalMeetCreateReq {
    #[serde(rename = "requestId")]
    request_id: String,
}

#[derive(Serialize)]
struct GCalMeetConferenceData {
    #[serde(rename = "createRequest")]
    create_request: GCalMeetCreateReq,
}

#[derive(Serialize)]
struct GCalMeetingBody<'a> {
    summary: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<&'a str>,
    start: GCalTimedDate<'a>,
    end: GCalTimedDate<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attendees: Option<Vec<GCalAttendeeInput>>,
    #[serde(rename = "conferenceData", skip_serializing_if = "Option::is_none")]
    conference_data: Option<GCalMeetConferenceData>,
}

pub async fn create_gcal_meeting(
    pool: &SqlitePool,
    dir: &Path,
    title: &str,
    date: &str,
    start_time: &str,
    end_time: &str,
    time_zone: &str,
    description: Option<&str>,
    location: Option<&str>,
    attendees: Vec<String>,
    add_meet: bool,
    zoom_url: Option<&str>,
) -> Result<serde_json::Value, String> {
    if !calendar_connected(dir) {
        return Err("Google Calendar is not connected".to_string());
    }
    let token = get_valid_access_token(dir).await?;

    let start_dt = format!("{date}T{start_time}:00");
    let end_dt = format!("{date}T{end_time}:00");

    let attendees_input: Option<Vec<GCalAttendeeInput>> = if attendees.is_empty() {
        None
    } else {
        Some(
            attendees
                .iter()
                .map(|e| GCalAttendeeInput { email: e.clone() })
                .collect(),
        )
    };

    // Prepend Zoom link to description when provided
    let zoom_description_prefix: String;
    let effective_description: Option<&str> = if let Some(url) = zoom_url {
        zoom_description_prefix = match description {
            Some(d) if !d.is_empty() => format!("Zoom Meeting: {url}\n\n{d}"),
            _ => format!("Zoom Meeting: {url}"),
        };
        Some(&zoom_description_prefix)
    } else {
        description
    };

    let conference_data = if add_meet {
        Some(GCalMeetConferenceData {
            create_request: GCalMeetCreateReq {
                request_id: Ulid::new().to_string(),
            },
        })
    } else {
        None
    };

    let body = GCalMeetingBody {
        summary: title,
        description: effective_description,
        location,
        start: GCalTimedDate {
            date_time: &start_dt,
            time_zone,
        },
        end: GCalTimedDate {
            date_time: &end_dt,
            time_zone,
        },
        attendees: attendees_input,
        conference_data,
    };

    let mut req = reqwest::Client::new()
        .post(format!("{GCAL_API}/calendars/primary/events"))
        .bearer_auth(&token)
        .query(&[("sendUpdates", "all")]);

    if add_meet {
        req = req.query(&[("conferenceDataVersion", "1")]);
    }

    let resp = req.json(&body).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(resp.text().await.unwrap_or_default());
    }

    #[derive(Deserialize)]
    struct CreatedEvent {
        id: String,
        #[serde(rename = "htmlLink")]
        html_link: Option<String>,
        #[serde(rename = "conferenceData")]
        conference_data: Option<serde_json::Value>,
    }

    let created = resp
        .json::<CreatedEvent>()
        .await
        .map_err(|e| e.to_string())?;

    // Extract Meet join link and flatten into the same schema sync_calendar uses
    let (meet_link, conf_json) = if let Some(ref cd) = created.conference_data {
        let video_uri = cd
            .get("entryPoints")
            .and_then(|e| e.as_array())
            .and_then(|arr| {
                arr.iter().find_map(|ep| {
                    if ep.get("entryPointType")?.as_str()? == "video" {
                        ep.get("uri")?.as_str().map(|s| s.to_string())
                    } else {
                        None
                    }
                })
            });
        let phone_uri = cd
            .get("entryPoints")
            .and_then(|e| e.as_array())
            .and_then(|arr| {
                arr.iter().find_map(|ep| {
                    if ep.get("entryPointType")?.as_str()? == "phone" {
                        ep.get("uri")?.as_str().map(|s| s.to_string())
                    } else {
                        None
                    }
                })
            });
        let phone_pin = cd
            .get("entryPoints")
            .and_then(|e| e.as_array())
            .and_then(|arr| {
                arr.iter().find_map(|ep| {
                    if ep.get("entryPointType")?.as_str()? == "phone" {
                        ep.get("pin")?.as_str().map(|s| s.to_string())
                    } else {
                        None
                    }
                })
            });
        let solution_name = cd
            .get("conferenceSolution")
            .and_then(|s| s.get("name"))
            .and_then(|n| n.as_str())
            .map(|s| s.to_string());
        let conf_id = cd
            .get("conferenceId")
            .and_then(|c| c.as_str())
            .map(|s| s.to_string());
        let flat = serde_json::json!({
            "video_uri": video_uri,
            "phone_uri": phone_uri,
            "phone_pin": phone_pin,
            "conference_id": conf_id,
            "solution_name": solution_name,
        });
        (video_uri, Some(flat.to_string()))
    } else {
        (None, None)
    };

    // Store attendees as JSON array matching the sync_calendar format
    let attendees_json = if attendees.is_empty() {
        None
    } else {
        Some(
            serde_json::to_string(
                &attendees
                    .iter()
                    .map(|e| serde_json::json!({ "email": e, "responseStatus": "needsAction" }))
                    .collect::<Vec<_>>(),
            )
            .unwrap_or_default(),
        )
    };

    let now_iso = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let local_id = Ulid::new().to_string();

    let _ = sqlx::query(
        "INSERT OR REPLACE INTO gcal_events
         (id, gcal_event_id, title, description, location, start_date, is_all_day,
          start_datetime, end_datetime, attendees, conference_data, updated_at)
         VALUES (?,?,?,?,?,?,0,?,?,?,?,?)",
    )
    .bind(&local_id)
    .bind(&created.id)
    .bind(title)
    .bind(description)
    .bind(location)
    .bind(date)
    .bind(&start_dt)
    .bind(&end_dt)
    .bind(&attendees_json)
    .bind(&conf_json)
    .bind(&now_iso)
    .execute(pool)
    .await;

    Ok(serde_json::json!({
        "success": true,
        "gcal_event_id": created.id,
        "html_link": created.html_link,
        "meet_link": meet_link,
        "title": title,
        "date": date,
    }))
}

/// Delete a GCal event by its event ID.
pub async fn tool_delete_calendar_event(
    pool: &SqlitePool,
    dir: &std::path::Path,
    gcal_event_id: &str,
) -> serde_json::Value {
    if !calendar_connected(dir) {
        return serde_json::json!({ "error": "Google Calendar is not connected" });
    }
    let token = match get_valid_access_token(dir).await {
        Ok(t) => t,
        Err(e) => return serde_json::json!({ "error": e }),
    };
    if let Err(e) = delete_event(&token, gcal_event_id).await {
        return serde_json::json!({ "error": e });
    }
    let _ = sqlx::query("DELETE FROM gcal_events WHERE gcal_event_id = ?")
        .bind(gcal_event_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM gcal_sync_map WHERE gcal_event_id = ?")
        .bind(gcal_event_id)
        .execute(pool)
        .await;
    serde_json::json!({ "success": true, "gcal_event_id": gcal_event_id, "deleted": true })
}
