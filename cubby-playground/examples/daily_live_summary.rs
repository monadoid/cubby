use std::env;

use anyhow::{anyhow, Context, Result};
use async_openai::{
    config::OpenAIConfig,
    types::responses::{
        Content, CreateResponseArgs, Input, InputContent, InputItem, InputMessageArgs,
        OutputContent, ResponseFormatJsonSchema, Role, TextConfig, TextResponseFormat,
    },
    Client,
};
use chrono::{DateTime, Duration as ChronoDuration, Local, NaiveDate, Utc};
use cubby_playground::{bootstrap, PlaygroundOptions};
use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{sqlite::SqliteRow, Row, SqlitePool};
use tiktoken_rs::o200k_base;
use toml;

const SYSTEM_PROMPT: &str = r#"
You are an expert daily activity analyst. You receive dense live-summary snapshots captured roughly once per second from a user's screen. Each snapshot already contains a concise detail field summarizing what was visible. Your job is to craft a day-in-review that helps the user remember meaningful work and stories that they could reuse in a recap, stand-up update, or blog post.

Core objectives:
- Preserve concrete specifics (repos, file paths, commands, error messages, terminology, referenced docs).
- Combine adjacent events into coherent chronological segments that describe what the user actually did.
- Trim obvious noise or repeated boilerplate while keeping novel discoveries or pivots.
- Highlight interesting problem-solving steps, research threads, or artifacts produced.
- If data is sparse or noisy, mention that limitation in the overview while still returning a valid JSON object.

Always follow the exact output schema given in the user message. Output MUST be valid JSON because it feeds another system. Avoid markdown, prefixes, or explanations."#;

const OUTPUT_SCHEMA_INSTRUCTIONS: &str = r#"
Return a JSON object with the fields:
{
  "overview": string,
  "bullet_points": [
    {
      "title": string,
      "summary": string,
      "notable_details": [string, ...],   // 1-4 concise, high-signal specifics
      "apps": [string, ...],              // unique app/window identifiers involved
      "confidence": number                // 0.0-1.0 subjective confidence in this bullet
    }
  ],
  "meta": {
    "date": "YYYY-MM-DD",
    "total_events": integer,
    "source": "live_summaries"
  }
}

Formatting requirements:
- Each bullet's summary should read like a single polished bullet sentence (no multiple paragraphs) and mention concrete artifacts (file names, URLs, commands, ticket IDs, etc.) whenever present.
- Use notable_details to surface key supporting facts (1-4 items) that the user might reuse directly (quotes, error codes, references consulted, concrete outcomes).
- Populate apps with distinct event_app values (or inferred app names) relevant to that segment; leave it empty only if nothing can be inferred.
- Keep bullet_points between 4 and 8 items when possible; if data is extremely sparse, return whatever number is reasonable and explain in the overview.
- Always return all required fields."#;

#[derive(Debug, Serialize, Deserialize)]
struct ModelEvent {
    frame_id: i64,
    utc_time: String,
    local_time: String,
    seconds_since_start: i64,
    label: String,
    detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    app: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    window: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ModelInput {
    date: String,
    timezone_offset: String,
    total_events: usize,
    source: &'static str,
    guidance: &'static str,
    events: Vec<ModelEvent>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct DailySummaryOutput {
    overview: String,
    bullet_points: Vec<SummaryBullet>,
    meta: SummaryMeta,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SummaryBullet {
    title: String,
    summary: String,
    notable_details: Vec<String>,
    apps: Vec<String>,
    confidence: f32,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SummaryMeta {
    date: String,
    total_events: i64,
    source: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let target_date = parse_target_date()?;
    let start = target_date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| anyhow!("invalid start time for {}", target_date))?
        .and_utc();
    let end = start + ChronoDuration::days(1);

    let ctx = bootstrap(PlaygroundOptions::default()).await?;
    let pool = &ctx.db().pool;

    let events = load_events(pool, start, end).await?;
    if events.is_empty() {
        println!(
            "{{\"overview\":\"No live summaries captured for {}.\",\"bullet_points\":[],\"meta\":{{\"date\":\"{}\",\"total_events\":0,\"source\":\"live_summaries\"}}}}",
            target_date, target_date
        );
        return Ok(());
    }

    let timezone_offset = Local::now().format("%:z").to_string();
    let payload = ModelInput {
        date: target_date.to_string(),
        timezone_offset,
        total_events: events.len(),
        source: "live_summaries",
        guidance: "Events are sampled roughly every second; group adjacent events into meaningful segments and keep high-signal specifics.",
        events,
    };

    let user_content = format!(
        "Summarize the user's day captured on {date}.\n\
Follow the output schema below:\n\
{schema}\n\
Data:\n```json\n{data}\n```",
        date = payload.date,
        schema = OUTPUT_SCHEMA_INSTRUCTIONS,
        data = serde_json::to_string_pretty(&payload)?
    );

    let text_config = summary_text_config();
    let instruction_text = SYSTEM_PROMPT.trim();
    let request = CreateResponseArgs::default()
        .model("gpt-5")
        .instructions(instruction_text.to_string())
        .text(text_config)
        .input(Input::Items(vec![InputItem::Message(
            InputMessageArgs::default()
                .role(Role::User)
                .content(InputContent::TextInput(user_content.clone()))
                .build()?,
        )]))
        .build()?;

    if let Some(tokens) = estimate_token_usage_parts(&[instruction_text, &user_content]) {
        let estimated_cost = (tokens as f64 / 1_000_000.0) * 1.25;
        println!(
            "Estimated input tokens: {} (~${:.4})",
            tokens, estimated_cost
        );
    } else {
        println!("Estimated input tokens: unavailable (model tokenizer not found)");
    }

    let api_key =
        env::var("OPENAI_API_KEY").context("Set OPENAI_API_KEY environment variable first")?;
    let api_base = env::var("OPENAI_API_BASE").ok();
    let mut config = OpenAIConfig::new().with_api_key(api_key);
    if let Some(base) = api_base {
        config = config.with_api_base(base);
    }
    let client = Client::with_config(config);

    println!("calling openai…");

    match client.responses().create(request).await {
        Ok(response) => {
            if let Some(summary) = extract_summary(&response.output)? {
                let json_value = serde_json::to_value(&summary)?;
                let toml_output = json_to_toml(&json_value)?;
                println!("{toml_output}");
            } else if let Some(text) = &response.output_text {
                println!("{text}");
            } else {
                let json_value = serde_json::to_value(&response)?;
                let toml_output = json_to_toml(&json_value)?;
                println!("{toml_output}");
            }
        }
        Err(err) => {
            eprintln!("OpenAI error: {err}");
        }
    }

    Ok(())
}

fn parse_target_date() -> Result<NaiveDate> {
    let mut args = env::args().skip(1);
    if let Some(date_str) = args.next() {
        NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
            .with_context(|| format!("expected date in YYYY-MM-DD format, got {}", date_str))
    } else {
        Ok((Local::now().date_naive()) - ChronoDuration::days(1))
    }
}

async fn load_events(
    pool: &SqlitePool,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<ModelEvent>> {
    let rows = sqlx::query(
        r#"
SELECT frame_id, event_time, event_label, event_detail, event_app, event_window
FROM live_summaries
WHERE event_time >= ?1 AND event_time < ?2
ORDER BY event_time ASC
"#,
    )
    .bind(start)
    .bind(end)
    .fetch_all(pool)
    .await?;

    let mut events = Vec::with_capacity(rows.len());
    for row in rows {
        if let Some(event) = convert_row(row, start) {
            events.push(event);
        }
    }

    Ok(events)
}

fn convert_row(row: SqliteRow, start: DateTime<Utc>) -> Option<ModelEvent> {
    let frame_id: i64 = row.try_get("frame_id").ok()?;
    let raw_time: String = row.try_get("event_time").ok()?;
    let parsed_time = DateTime::parse_from_rfc3339(&raw_time).ok()?;
    let event_time = parsed_time.with_timezone(&Utc);
    let label: String = row.try_get("event_label").ok()?;
    let detail: String = row.try_get("event_detail").ok()?;

    let seconds_since_start = event_time.signed_duration_since(start).num_seconds().max(0);

    let local_time = event_time
        .with_timezone(&Local)
        .format("%Y-%m-%dT%H:%M:%S%:z")
        .to_string();

    Some(ModelEvent {
        frame_id,
        utc_time: event_time.to_rfc3339(),
        local_time,
        seconds_since_start,
        label,
        detail: detail.trim().to_string(),
        app: row.try_get("event_app").ok().and_then(normalize_opt_string),
        window: row
            .try_get("event_window")
            .ok()
            .and_then(normalize_opt_string),
    })
}

fn normalize_opt_string(value: Option<String>) -> Option<String> {
    value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn summary_text_config() -> TextConfig {
    let schema = schema_for!(DailySummaryOutput);
    let mut schema_value = serde_json::to_value(schema)
        .unwrap_or_else(|_| json!({ "type": "object", "additionalProperties": false }));

    // Inline all $refs to flatten the schema
    inline_refs(&mut schema_value);

    // Fix nested schemas to ensure they have 'required' arrays
    // OpenAI requires every object with properties to have a 'required' field
    fix_schema_required(&mut schema_value);

    TextConfig {
        format: TextResponseFormat::JsonSchema(ResponseFormatJsonSchema {
            description: Some(
                "Daily digest of live summary events with structured bullet points.".to_string(),
            ),
            name: "daily_live_summary".to_string(),
            schema: Some(schema_value),
            strict: Some(true),
        }),
        verbosity: None,
    }
}

fn inline_refs(value: &mut serde_json::Value) {
    if let serde_json::Value::Object(obj) = value {
        // Get the $defs/definitions section for reference resolution
        let defs = obj
            .get("$defs")
            .or_else(|| obj.get("definitions"))
            .and_then(|v| v.as_object());

        if let Some(definitions) = defs {
            // Clone definitions to avoid borrow checker issues
            let definitions_clone = definitions.clone();
            // Replace $ref references with actual definitions
            inline_refs_recursive(value, &definitions_clone);
        }
    }
}

fn inline_refs_recursive(
    value: &mut serde_json::Value,
    definitions: &serde_json::Map<String, serde_json::Value>,
) {
    match value {
        serde_json::Value::Object(obj) => {
            // Check if this object has a $ref
            if let Some(serde_json::Value::String(ref_path)) = obj.get("$ref") {
                if let Some((_, name)) = ref_path.rsplit_once('#') {
                    if let Some(def_path) = name.strip_prefix("/definitions/") {
                        if let Some(def_val) = definitions.get(def_path) {
                            // Clone the definition and inline it
                            *value = def_val.clone();
                            // Recursively inline any nested refs
                            inline_refs_recursive(value, definitions);
                        }
                    }
                }
            } else {
                // Recursively process all values
                for val in obj.values_mut() {
                    inline_refs_recursive(val, definitions);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr.iter_mut() {
                inline_refs_recursive(item, definitions);
            }
        }
        _ => {}
    }
}

fn fix_schema_required(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(obj) => {
            // If this object has properties, ensure it has a required field
            if obj.contains_key("properties") && !obj.contains_key("required") {
                obj.insert("required".to_string(), json!([]));
            }

            // Recursively fix all nested values
            for val in obj.values_mut() {
                fix_schema_required(val);
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr.iter_mut() {
                fix_schema_required(item);
            }
        }
        _ => {}
    }
}

fn estimate_token_usage_parts(chunks: &[&str]) -> Option<usize> {
    let bpe = o200k_base().ok()?;
    Some(
        chunks
            .iter()
            .map(|chunk| bpe.encode_with_special_tokens(chunk).len())
            .sum(),
    )
}

fn json_to_toml(json: &serde_json::Value) -> Result<String> {
    let toml_value = json_to_toml_value(json)?;
    Ok(toml::to_string_pretty(&toml_value)?)
}

fn json_to_toml_value(value: &serde_json::Value) -> Result<toml::Value> {
    match value {
        serde_json::Value::Null => Ok(toml::Value::String("null".to_string())),
        serde_json::Value::Bool(b) => Ok(toml::Value::Boolean(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(toml::Value::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(toml::Value::Float(f))
            } else {
                Ok(toml::Value::String(n.to_string()))
            }
        }
        serde_json::Value::String(s) => Ok(toml::Value::String(s.clone())),
        serde_json::Value::Array(arr) => {
            let toml_array = arr
                .iter()
                .map(json_to_toml_value)
                .collect::<Result<Vec<_>>>()?;
            Ok(toml::Value::Array(toml_array))
        }
        serde_json::Value::Object(obj) => {
            let mut toml_table = toml::Table::new();
            for (key, val) in obj {
                toml_table.insert(key.clone(), json_to_toml_value(val)?);
            }
            Ok(toml::Value::Table(toml_table))
        }
    }
}

fn extract_summary(output: &[OutputContent]) -> Result<Option<DailySummaryOutput>> {
    for item in output {
        if let OutputContent::Message(message) = item {
            for part in &message.content {
                if let Content::OutputText(text) = part {
                    let summary: DailySummaryOutput = serde_json::from_str(&text.text)?;
                    return Ok(Some(summary));
                }
            }
        }
    }
    Ok(None)
}
