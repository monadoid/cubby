use std::{env, future::Future, pin::Pin};

use anyhow::{anyhow, bail, Context, Result};
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestSystemMessageContent,
        ChatCompletionRequestUserMessageArgs, ChatCompletionRequestUserMessageContent,
        CreateChatCompletionRequestArgs, CreateEmbeddingRequestArgs, ResponseFormat,
        ResponseFormatJsonSchema,
    },
    Client,
};
use schemars::{schema_for, JsonSchema};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{self, Value};
use std::fmt::Debug;

// ---------- Core data models ----------

#[derive(Clone, Debug)]
pub struct Example<I, T> {
    pub input: I,
    pub target: T,
}

#[derive(Clone, Debug)]
pub struct Dataset<I, T> {
    pub train: Vec<Example<I, T>>,
    pub dev: Vec<Example<I, T>>, // held-out for early stop / model selection
    pub test: Vec<Example<I, T>>, // final reporting
}

#[derive(Clone, Debug)]
pub struct PromptTemplate {
    /// System message (policies, persona, output rules)
    pub system: String,
    /// User prompt template. Include "{{input_json}}" where the input will be injected.
    /// (Keep it model-agnostic—no special templating engine required.)
    pub user_template: String,
}

/// Minimal chat message for provider-agnostic calls.
#[derive(Clone, Debug, Serialize)]
pub struct ChatMessage {
    pub role: String, // "system" | "user" | "assistant"
    pub content: String,
}

// ---------- Pluggable scoring ----------

/// Generic scorer over deserialized outputs of type T (same shape as your `target`).
pub trait Scorer<T> {
    /// Return a score in [0,1]; higher is better.
    fn score(&self, predicted: &T, target: &T) -> f32;
}

/// Exact JSON equality (handy for strict normalized JSON targets).
pub struct ExactScorer;
impl<T: Serialize + DeserializeOwned + PartialEq> Scorer<T> for ExactScorer {
    fn score(&self, predicted: &T, target: &T) -> f32 {
        if predicted == target {
            1.0
        } else {
            0.0
        }
    }
}

// Example field-wise scorer (replace with your own logic as needed).
pub struct FieldWiseFn<T>(pub Box<dyn Fn(&T, &T) -> f32 + Send + Sync>);
impl<T> Scorer<T> for FieldWiseFn<T> {
    fn score(&self, predicted: &T, target: &T) -> f32 {
        (self.0)(predicted, target)
    }
}

// ---------- Provider-agnostic LLM client ----------

type LlmFuture<'a, T> = Pin<Box<dyn Future<Output = anyhow::Result<T>> + Send + 'a>>;

#[derive(Clone, Debug)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: f32,
    /// Optional: set an explicit response format (e.g., JSON mode, structured schema).
    pub response_format: Option<ResponseFormat>,
}

#[derive(Clone, Debug)]
pub struct ChatResponse {
    pub text: String,
}

/// Optional: embeddings for semantic similarity scoring.
#[derive(Clone, Debug)]
pub struct EmbeddingsRequest {
    pub model: String,
    pub inputs: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct EmbeddingsResponse {
    pub vectors: Vec<Vec<f32>>,
}

pub trait LlmClient: Send + Sync {
    fn chat<'a>(&'a self, req: ChatRequest) -> LlmFuture<'a, ChatResponse>;

    /// Optional—only if you want semantic similarity (cosine) as a fallback metric.
    fn embeddings<'a>(&'a self, _req: EmbeddingsRequest) -> LlmFuture<'a, EmbeddingsResponse> {
        Box::pin(async move { bail!("embeddings not implemented") })
    }
}

#[derive(Clone)]
pub struct AsyncOpenAiClient {
    client: Client<OpenAIConfig>,
    pub chat_model: String,
    pub embed_model: String,
}

impl AsyncOpenAiClient {
    pub fn new(client: Client<OpenAIConfig>, chat_model: String, embed_model: String) -> Self {
        Self {
            client,
            chat_model,
            embed_model,
        }
    }
}

impl LlmClient for AsyncOpenAiClient {
    fn chat<'a>(&'a self, req: ChatRequest) -> LlmFuture<'a, ChatResponse> {
        let client = self.client.clone();
        let fallback_model = self.chat_model.clone();
        Box::pin(async move {
            let ChatRequest {
                model,
                messages,
                temperature,
                response_format,
            } = req;
            let chosen_model = if model.is_empty() {
                fallback_model
            } else {
                model
            };
            let mapped = build_chat_messages(&messages)?;

            let mut builder = CreateChatCompletionRequestArgs::default();
            builder.model(chosen_model);
            builder.messages(mapped);
            builder.temperature(temperature);
            if let Some(format) = response_format {
                builder.response_format(format);
            }
            let request = builder.build().map_err(anyhow::Error::from)?;
            let response = client
                .chat()
                .create(request)
                .await
                .map_err(anyhow::Error::from)?;
            let text = response
                .choices
                .into_iter()
                .find_map(|choice| choice.message.content)
                .unwrap_or_default();

            Ok(ChatResponse { text })
        })
    }

    fn embeddings<'a>(&'a self, req: EmbeddingsRequest) -> LlmFuture<'a, EmbeddingsResponse> {
        let client = self.client.clone();
        Box::pin(async move {
            let EmbeddingsRequest { model, inputs } = req;
            let request = CreateEmbeddingRequestArgs::default()
                .model(model)
                .input(inputs)
                .build()
                .map_err(anyhow::Error::from)?;
            let response = client
                .embeddings()
                .create(request)
                .await
                .map_err(anyhow::Error::from)?;
            let vectors = response.data.into_iter().map(|row| row.embedding).collect();
            Ok(EmbeddingsResponse { vectors })
        })
    }
}

fn build_chat_messages(messages: &[ChatMessage]) -> Result<Vec<ChatCompletionRequestMessage>> {
    messages
        .iter()
        .map(|msg| {
            let role = msg.role.to_lowercase();
            match role.as_str() {
                "system" => ChatCompletionRequestSystemMessageArgs::default()
                    .content(ChatCompletionRequestSystemMessageContent::Text(
                        msg.content.clone(),
                    ))
                    .build()
                    .map(Into::into)
                    .map_err(anyhow::Error::from),
                "user" => ChatCompletionRequestUserMessageArgs::default()
                    .content(ChatCompletionRequestUserMessageContent::Text(
                        msg.content.clone(),
                    ))
                    .build()
                    .map(Into::into)
                    .map_err(anyhow::Error::from),
                "assistant" => ChatCompletionRequestAssistantMessageArgs::default()
                    .content(msg.content.clone())
                    .build()
                    .map(Into::into)
                    .map_err(anyhow::Error::from),
                other => Err(anyhow!("unsupported role: {other}")),
            }
        })
        .collect()
}

// ---------- Rendering + evaluation ----------

/// Render messages for the model, including the JSON Schema of T to force strict JSON.
pub fn render_messages<I, T>(prompt: &PromptTemplate, input: &I) -> Result<Vec<ChatMessage>>
where
    I: Serialize + JsonSchema,
    T: Serialize + DeserializeOwned + JsonSchema,
{
    let input_json = serde_json::to_string(input)?;
    let target_schema = schema_for!(T);
    let schema_str = serde_json::to_string(&target_schema.schema)?;
    let user = prompt.user_template.replace("{{input_json}}", &input_json);

    let system = format!(
        "You are a data normalizer. Produce JSON that matches this JSON Schema (no markdown, no prose):\n{}",
        schema_str
    );

    Ok(vec![
        ChatMessage {
            role: "system".to_string(),
            content: format!("{}\n\n{}", prompt.system, system),
        },
        ChatMessage {
            role: "user".to_string(),
            content: user,
        },
    ])
}

/// Call the LLM and parse strict JSON into T.
pub async fn generate<T, C>(client: &C, model: &str, messages: Vec<ChatMessage>) -> Result<T>
where
    T: Serialize + DeserializeOwned + JsonSchema,
    C: LlmClient,
{
    let resp = client
        .chat(ChatRequest {
            model: model.to_string(),
            messages,
            temperature: 0.0,
            response_format: Some(ResponseFormat::JsonObject),
        })
        .await?;

    let parsed: T = serde_json::from_str(&resp.text)
        .map_err(|e| anyhow::anyhow!("Failed to parse model JSON: {e}\nRAW: {}", resp.text))?;
    Ok(parsed)
}

/// Evaluate a prompt on a batch and return mean score in [0,1].
pub async fn eval_prompt<I, T, C, S>(
    client: &C,
    model: &str,
    prompt: &PromptTemplate,
    batch: &[Example<I, T>],
    scorer: &S,
) -> Result<f32>
where
    I: Serialize + JsonSchema + Sync,
    T: Serialize + DeserializeOwned + JsonSchema + Sync,
    C: LlmClient + Sync,
    S: Scorer<T> + Sync,
{
    let mut total = 0.0_f32;
    for ex in batch {
        let msgs = render_messages::<I, T>(prompt, &ex.input)?;
        let y_hat: T = generate::<T, C>(client, model, msgs).await?;
        total += scorer.score(&y_hat, &ex.target);
    }
    Ok(total / (batch.len().max(1) as f32))
}

// ---------- Prompt proposal (auto-edits) ----------

/// A proposer that asks the LLM to suggest small edits to the current prompt.
pub struct LlmProposer<'a, C> {
    pub client: &'a C,
    pub model: &'a str,
    /// How many candidates to propose per round.
    pub k: usize,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PromptSuggestion {
    system: String,
    user_template: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PromptSuggestionSet {
    prompts: Vec<PromptSuggestion>,
}

impl<'a, C: LlmClient + Sync> LlmProposer<'a, C> {
    pub async fn propose(
        &self,
        current: &PromptTemplate,
        recent_notes: &str,
    ) -> Result<Vec<PromptTemplate>> {
        let system = "You rewrite developer prompts. Return JSON matching the provided schema.";
        let user = format!(
            "k = {}\nCURRENT_SYSTEM:\n{}\n\nCURRENT_USER_TEMPLATE:\n{}\n\nNOTES (failures, error modes, fields often wrong):\n{}\n\nConstraints:\n- Keep intent.\n- Prefer clarifying output schema, field rules, edge cases.\n- Avoid verbosity.\n- No markdown in outputs from the model.",
            self.k, current.system, current.user_template, recent_notes
        );

        let messages = vec![
            ChatMessage {
                role: "system".into(),
                content: system.into(),
            },
            ChatMessage {
                role: "user".into(),
                content: user,
            },
        ];

        let schema = schema_for!(PromptSuggestionSet);
        let mut schema_value =
            serde_json::to_value(schema).unwrap_or_else(|_| Value::Object(Default::default()));
        inline_refs(&mut schema_value);
        fix_schema_required(&mut schema_value);
        enforce_no_additional_properties(&mut schema_value);
        if let Value::Object(root) = &mut schema_value {
            if let Some(Value::Object(props)) = root.get_mut("properties") {
                if let Some(Value::Object(prompts)) = props.get_mut("prompts") {
                    prompts
                        .entry("minItems".to_string())
                        .or_insert(Value::from(self.k.max(1) as u64));
                }
            }
        }
        let response_format = ResponseFormat::JsonSchema {
            json_schema: ResponseFormatJsonSchema {
                name: "prompt_suggestions".into(),
                description: Some("List of improved prompts with small edits".into()),
                schema: Some(schema_value),
                strict: Some(true),
            },
        };

        let resp = self
            .client
            .chat(ChatRequest {
                model: self.model.to_string(),
                messages,
                temperature: 0.3,
                response_format: Some(response_format),
            })
            .await?;

        let suggestions: PromptSuggestionSet = serde_json::from_str(&resp.text)?;
        let mut out: Vec<PromptTemplate> = suggestions
            .prompts
            .into_iter()
            .map(|c| PromptTemplate {
                system: c.system,
                user_template: c.user_template,
            })
            .collect();
        if out.len() > self.k {
            out.truncate(self.k);
        } else if out.len() < self.k {
            eprintln!(
                "warning: proposer returned {} prompts but k={}; consider inspecting response: {}",
                out.len(),
                self.k,
                resp.text
            );
        }
        Ok(out)
    }
}

fn inline_refs(value: &mut Value) {
    let defs = match value {
        Value::Object(obj) => obj
            .get("$defs")
            .or_else(|| obj.get("definitions"))
            .and_then(|v| v.as_object())
            .cloned(),
        _ => None,
    };
    if let Some(definitions) = defs {
        inline_refs_recursive(value, &definitions);
    }
    if let Value::Object(obj) = value {
        obj.remove("$defs");
        obj.remove("definitions");
    }
}

fn inline_refs_recursive(value: &mut Value, definitions: &serde_json::Map<String, Value>) {
    match value {
        Value::Object(obj) => {
            if let Some(Value::String(ref_path)) = obj.get("$ref") {
                if let Some((_, fragment)) = ref_path.rsplit_once('#') {
                    if let Some(name) = fragment.strip_prefix("/definitions/") {
                        if let Some(def_val) = definitions.get(name) {
                            *value = def_val.clone();
                            inline_refs_recursive(value, definitions);
                            return;
                        }
                    }
                    if let Some(name) = fragment.strip_prefix("/$defs/") {
                        if let Some(def_val) = definitions.get(name) {
                            *value = def_val.clone();
                            inline_refs_recursive(value, definitions);
                            return;
                        }
                    }
                }
            }
            for val in obj.values_mut() {
                inline_refs_recursive(val, definitions);
            }
        }
        Value::Array(items) => {
            for item in items {
                inline_refs_recursive(item, definitions);
            }
        }
        _ => {}
    }
}

fn fix_schema_required(value: &mut Value) {
    fix_schema_required_recursive(value, false);
}

fn fix_schema_required_recursive(value: &mut Value, parent_is_array: bool) {
    match value {
        Value::Object(obj) => {
            let object_type = matches!(obj.get("type"), Some(Value::String(s)) if s == "object");
            if object_type {
                if !obj.contains_key("required") {
                    if let Some(Value::Object(props)) = obj.get("properties") {
                        let keys: Vec<Value> = props.keys().cloned().map(Value::String).collect();
                        obj.insert("required".to_string(), Value::Array(keys));
                    } else {
                        obj.insert("required".to_string(), Value::Array(vec![]));
                    }
                }
            }

            let array_type = matches!(obj.get("type"), Some(Value::String(s)) if s == "array");
            if array_type {
                if let Some(items) = obj.get_mut("items") {
                    fix_schema_required_recursive(items, true);
                }
            }

            for val in obj.values_mut() {
                fix_schema_required_recursive(val, false);
            }
        }
        Value::Array(arr) => {
            for val in arr {
                fix_schema_required_recursive(val, parent_is_array);
            }
        }
        _ => {}
    }
}

fn enforce_no_additional_properties(value: &mut Value) {
    match value {
        Value::Object(obj) => {
            if matches!(obj.get("type"), Some(Value::String(s)) if s == "object") {
                obj.entry("additionalProperties")
                    .or_insert(Value::Bool(false));
            }
            for val in obj.values_mut() {
                enforce_no_additional_properties(val);
            }
        }
        Value::Array(arr) => {
            for val in arr {
                enforce_no_additional_properties(val);
            }
        }
        _ => {}
    }
}

// ---------- Optimization loop (hill-climb with holdout) ----------

pub struct OptimizeConfig {
    pub rounds: usize, // max rounds
    pub proposals_per_round: usize,
    pub train_batch_size: usize, // subsample if your train set is large
    pub patience: usize,         // early stop rounds without improvement
}

pub struct OptimizeReport {
    pub best_prompt: PromptTemplate,
    pub dev_score: f32,
    pub test_score: f32,
    pub history: Vec<(usize, f32)>, // (round, dev_score)
}

pub async fn optimize<I, T, C, S>(
    client: &C,
    model: &str,
    data: &Dataset<I, T>,
    initial: PromptTemplate,
    scorer: &S,
    proposer: &LlmProposer<'_, C>,
    cfg: OptimizeConfig,
) -> Result<OptimizeReport>
where
    I: Serialize + JsonSchema + Sync + Clone,
    T: Serialize + DeserializeOwned + JsonSchema + Sync + Clone,
    C: LlmClient + Sync,
    S: Scorer<T> + Sync,
{
    let mut best = initial.clone();
    let mut best_dev = eval_prompt(client, model, &best, &data.dev, scorer).await?;
    let mut history = vec![(0, best_dev)];
    let mut notes = String::new();
    let mut rounds_since_improve = 0;

    for r in 1..=cfg.rounds {
        // Get candidates
        let mut cands = proposer.propose(&best, &notes).await?;
        if cands.len() > cfg.proposals_per_round {
            cands.truncate(cfg.proposals_per_round);
        }

        // (Optional) subsample train for speed
        let train_batch: Vec<_> = data
            .train
            .iter()
            .take(cfg.train_batch_size.min(data.train.len()))
            .map(|ex| ex.clone())
            .collect();

        // Evaluate
        let mut best_round_score = best_dev;
        let mut best_round_prompt = best.clone();

        for cand in cands {
            let train_score = eval_prompt(client, model, &cand, &train_batch, scorer).await?;
            // Keep the candidate if it beats current best_dev on the full dev set.
            if train_score >= best_round_score {
                let dev_score = eval_prompt(client, model, &cand, &data.dev, scorer).await?;
                if dev_score > best_round_score {
                    best_round_score = dev_score;
                    best_round_prompt = cand.clone();
                }
            }
        }

        if best_round_score > best_dev {
            best = best_round_prompt;
            best_dev = best_round_score;
            rounds_since_improve = 0;
        } else {
            rounds_since_improve += 1;
        }

        history.push((r, best_dev));
        if rounds_since_improve >= cfg.patience {
            break;
        }

        // Optional: append brief failure notes to guide proposer (manually or via quick stats)
        notes = format!("dev_score={:.3} after round {}", best_dev, r);
    }

    let test_score = eval_prompt(client, model, &best, &data.test, scorer).await?;
    Ok(OptimizeReport {
        best_prompt: best,
        dev_score: best_dev,
        test_score,
        history,
    })
}

// ---------- Example: wire it up ----------

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
struct OcrFrame {
    text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
struct Normalized {
    app: String,
    site: String,
    topic: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let api_key =
        env::var("OPENAI_API_KEY").context("Set OPENAI_API_KEY environment variable first")?;
    let api_base = env::var("OPENAI_API_BASE").ok();
    let chat_model = env::var("OPENAI_CHAT_MODEL").unwrap_or_else(|_| "gpt-4.1-mini".into());
    let embed_model =
        env::var("OPENAI_EMBED_MODEL").unwrap_or_else(|_| "text-embedding-3-large".into());

    let mut config = OpenAIConfig::new().with_api_key(api_key);
    if let Some(base) = api_base {
        config = config.with_api_base(base);
    }
    let raw_client = Client::with_config(config);
    let client = AsyncOpenAiClient::new(raw_client, chat_model.clone(), embed_model.clone());

    let data = Dataset {
        train: vec![Example {
            input: OcrFrame {
                text: "Firefox: Wikipedia - Elephant".into(),
            },
            target: Normalized {
                app: "Firefox".into(),
                site: "en.wikipedia.org".into(),
                topic: "elephants".into(),
            },
        }],
        dev: vec![Example {
            input: OcrFrame {
                text: "Chrome: NYTimes - Climate Change".into(),
            },
            target: Normalized {
                app: "Chrome".into(),
                site: "nytimes.com".into(),
                topic: "climate change".into(),
            },
        }],
        test: vec![],
    };

    let prompt0 = PromptTemplate {
        system: "Extract structured facts from OCR text. Reply with STRICT JSON only.".into(),
        user_template: r#"Input (JSON):
{{input_json}}

Task:
Return a JSON object with fields: app, site, topic."#
            .into(),
    };

    let scorer = ExactScorer;
    let proposer = LlmProposer {
        client: &client,
        model: chat_model.as_str(),
        k: 2,
    };

    let report = optimize(
        &client,
        chat_model.as_str(),
        &data,
        prompt0,
        &scorer,
        &proposer,
        OptimizeConfig {
            rounds: 1,
            proposals_per_round: 1,
            train_batch_size: 1,
            patience: 1,
        },
    )
    .await?;

    println!(
        "dev_score={:.3} test_score={:.3}",
        report.dev_score, report.test_score
    );
    println!(
        "best system:\n{}\n\nbest user_template:\n{}",
        report.best_prompt.system, report.best_prompt.user_template
    );
    println!("history: {}", serde_json::to_string(&report.history)?);
    Ok(())
}
