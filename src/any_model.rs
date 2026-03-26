//! Unified multi-provider model abstraction for rig.
//!
//! This module provides [`AnyModel`], an enum-dispatch wrapper that implements
//! [`rig::completion::CompletionModel`]. It allows runtime selection of any
//! supported provider while exposing a single concrete type, so you can
//! feed it into [`AgentBuilder`], [`Prompt`](rig::completion::Prompt),
//! [`Chat`](rig::completion::Chat), etc.
//!
//! Each provider is gated behind a cargo feature flag.
//! Enable only the backends you need to keep binary size small
//! (especially important for future WASM targets).

use rig::agent::AgentBuilder;
use rig::client::{CompletionClient, ProviderClient};
use rig::completion::{
    CompletionError, CompletionModel, CompletionRequest, CompletionResponse, GetTokenUsage,
};
use rig::streaming::StreamingCompletionResponse;
use serde::{Deserialize, Serialize};
use std::env;

#[cfg(feature = "anthropic")]
use rig::providers::anthropic;
#[cfg(feature = "azure")]
use rig::providers::azure;
#[cfg(feature = "cohere")]
use rig::providers::cohere;
#[cfg(feature = "deepseek")]
use rig::providers::deepseek;
#[cfg(feature = "galadriel")]
use rig::providers::galadriel;
#[cfg(feature = "gemini")]
use rig::providers::gemini;
#[cfg(feature = "groq")]
use rig::providers::groq;
#[cfg(feature = "huggingface")]
use rig::providers::huggingface;
#[cfg(feature = "hyperbolic")]
use rig::providers::hyperbolic;
#[cfg(feature = "llamafile")]
use rig::providers::llamafile;
#[cfg(feature = "mira")]
use rig::providers::mira;
#[cfg(feature = "mistral")]
use rig::providers::mistral;
#[cfg(feature = "moonshot")]
use rig::providers::moonshot;
#[cfg(feature = "ollama")]
use rig::providers::ollama;
#[cfg(feature = "openai")]
use rig::providers::openai;
#[cfg(feature = "openrouter")]
use rig::providers::openrouter;
#[cfg(feature = "perplexity")]
use rig::providers::perplexity;
#[cfg(feature = "together")]
use rig::providers::together;
#[cfg(feature = "xai")]
use rig::providers::xai;

/// A unified completion model that dispatches to any supported provider
/// at runtime. Implements [`CompletionModel`] so it plugs directly into
/// [`AgentBuilder`], [`Prompt`](rig::completion::Prompt), etc.
#[derive(Clone)]
pub enum AnyModel {
    #[cfg(feature = "anthropic")]
    Anthropic(anthropic::completion::CompletionModel),
    #[cfg(feature = "azure")]
    Azure(azure::CompletionModel),
    #[cfg(feature = "cohere")]
    Cohere(cohere::completion::CompletionModel),
    #[cfg(feature = "deepseek")]
    Deepseek(deepseek::CompletionModel),
    #[cfg(feature = "galadriel")]
    Galadriel(galadriel::CompletionModel),
    #[cfg(feature = "gemini")]
    Gemini(gemini::completion::CompletionModel),
    #[cfg(feature = "groq")]
    Groq(groq::CompletionModel),
    #[cfg(feature = "huggingface")]
    Huggingface(huggingface::completion::CompletionModel),
    #[cfg(feature = "hyperbolic")]
    Hyperbolic(hyperbolic::CompletionModel),
    #[cfg(feature = "llamafile")]
    Llamafile(llamafile::CompletionModel),
    #[cfg(feature = "mira")]
    Mira(mira::CompletionModel),
    #[cfg(feature = "mistral")]
    Mistral(mistral::completion::CompletionModel),
    #[cfg(feature = "moonshot")]
    Moonshot(moonshot::CompletionModel),
    #[cfg(feature = "ollama")]
    Ollama(ollama::CompletionModel),
    #[cfg(feature = "openai")]
    OpenAi(openai::completion::CompletionModel),
    #[cfg(feature = "openrouter")]
    OpenRouter(openrouter::completion::CompletionModel),
    #[cfg(feature = "perplexity")]
    Perplexity(perplexity::CompletionModel),
    #[cfg(feature = "together")]
    Together(together::completion::CompletionModel),
    #[cfg(feature = "xai")]
    Xai(xai::completion::CompletionModel),
}

/// Type-erased streaming response used by [`AnyModel`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnyStreamingResponse {
    pub usage: Option<rig::completion::Usage>,
}

impl GetTokenUsage for AnyStreamingResponse {
    fn token_usage(&self) -> Option<rig::completion::Usage> {
        self.usage
    }
}

macro_rules! dispatch_completion {
    ($self:expr, $req:expr) => {
        match $self {
            #[cfg(feature = "anthropic")]
            AnyModel::Anthropic(m) => convert_response(m.completion($req).await?),
            #[cfg(feature = "azure")]
            AnyModel::Azure(m) => convert_response(m.completion($req).await?),
            #[cfg(feature = "cohere")]
            AnyModel::Cohere(m) => convert_response(m.completion($req).await?),
            #[cfg(feature = "deepseek")]
            AnyModel::Deepseek(m) => convert_response(m.completion($req).await?),
            #[cfg(feature = "galadriel")]
            AnyModel::Galadriel(m) => convert_response(m.completion($req).await?),
            #[cfg(feature = "gemini")]
            AnyModel::Gemini(m) => convert_response(m.completion($req).await?),
            #[cfg(feature = "groq")]
            AnyModel::Groq(m) => convert_response(m.completion($req).await?),
            #[cfg(feature = "huggingface")]
            AnyModel::Huggingface(m) => convert_response(m.completion($req).await?),
            #[cfg(feature = "hyperbolic")]
            AnyModel::Hyperbolic(m) => convert_response(m.completion($req).await?),
            #[cfg(feature = "llamafile")]
            AnyModel::Llamafile(m) => convert_response(m.completion($req).await?),
            #[cfg(feature = "mira")]
            AnyModel::Mira(m) => convert_response(m.completion($req).await?),
            #[cfg(feature = "mistral")]
            AnyModel::Mistral(m) => convert_response(m.completion($req).await?),
            #[cfg(feature = "moonshot")]
            AnyModel::Moonshot(m) => convert_response(m.completion($req).await?),
            #[cfg(feature = "ollama")]
            AnyModel::Ollama(m) => convert_response(m.completion($req).await?),
            #[cfg(feature = "openai")]
            AnyModel::OpenAi(m) => convert_response(m.completion($req).await?),
            #[cfg(feature = "openrouter")]
            AnyModel::OpenRouter(m) => convert_response(m.completion($req).await?),
            #[cfg(feature = "perplexity")]
            AnyModel::Perplexity(m) => convert_response(m.completion($req).await?),
            #[cfg(feature = "together")]
            AnyModel::Together(m) => convert_response(m.completion($req).await?),
            #[cfg(feature = "xai")]
            AnyModel::Xai(m) => convert_response(m.completion($req).await?),
        }
    };
}

// TODO: dispatch_stream macro commented out — rig's StreamingCompletionResponse
// doesn't expose a map/transform API, so we can't re-type the inner stream.
// Uncomment and implement when rig adds support.

fn convert_response<T: Serialize>(
    resp: CompletionResponse<T>,
) -> Result<CompletionResponse<serde_json::Value>, CompletionError> {
    Ok(CompletionResponse {
        choice: resp.choice,
        usage: resp.usage,
        raw_response: serde_json::to_value(&resp.raw_response).unwrap_or(serde_json::Value::Null),
        message_id: resp.message_id,
    })
}

impl CompletionModel for AnyModel {
    type Response = serde_json::Value;
    type StreamingResponse = AnyStreamingResponse;
    type Client = ();

    fn make(_client: &Self::Client, _model: impl Into<String>) -> Self {
        panic!(
            "AnyModel::make() is not supported. \
             Use AnyModel::from_env() or AnyModel::new(provider, model) instead."
        );
    }

    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<Self::Response>, CompletionError> {
        dispatch_completion!(self, request)
    }

    async fn stream(
        &self,
        _request: CompletionRequest,
    ) -> Result<StreamingCompletionResponse<Self::StreamingResponse>, CompletionError> {
        panic!(
            "AnyModel::stream() is not supported. \
             Use the concrete provider model type for streaming."
        );
    }
}

impl AnyModel {
    /// Create an [`AnyModel`] by provider name and model name.
    ///
    /// Provider names are case-insensitive. The corresponding feature flag
    /// must be enabled at compile time or this will panic.
    ///
    /// # Example
    /// ```rust,ignore
    /// let model = AnyModel::new("gemini", "gemini-2.5-flash");
    /// let model = AnyModel::new("openai", "gpt-4o");
    /// ```
    pub fn new(provider: &str, model_name: &str) -> Self {
        match provider.to_lowercase().as_str() {
            #[cfg(feature = "anthropic")]
            "anthropic" => {
                AnyModel::Anthropic(anthropic::Client::from_env().completion_model(model_name))
            }
            #[cfg(feature = "azure")]
            "azure" => AnyModel::Azure(azure::Client::from_env().completion_model(model_name)),
            #[cfg(feature = "cohere")]
            "cohere" => AnyModel::Cohere(cohere::Client::from_env().completion_model(model_name)),
            #[cfg(feature = "deepseek")]
            "deepseek" => {
                AnyModel::Deepseek(deepseek::Client::from_env().completion_model(model_name))
            }
            #[cfg(feature = "galadriel")]
            "galadriel" => {
                AnyModel::Galadriel(galadriel::Client::from_env().completion_model(model_name))
            }
            #[cfg(feature = "gemini")]
            "gemini" => AnyModel::Gemini(gemini::Client::from_env().completion_model(model_name)),
            #[cfg(feature = "groq")]
            "groq" => AnyModel::Groq(groq::Client::from_env().completion_model(model_name)),
            #[cfg(feature = "huggingface")]
            "huggingface" => {
                AnyModel::Huggingface(huggingface::Client::from_env().completion_model(model_name))
            }
            #[cfg(feature = "hyperbolic")]
            "hyperbolic" => {
                AnyModel::Hyperbolic(hyperbolic::Client::from_env().completion_model(model_name))
            }
            #[cfg(feature = "llamafile")]
            "llamafile" => {
                AnyModel::Llamafile(llamafile::Client::from_env().completion_model(model_name))
            }
            #[cfg(feature = "mira")]
            "mira" => AnyModel::Mira(mira::Client::from_env().completion_model(model_name)),
            #[cfg(feature = "mistral")]
            "mistral" => {
                AnyModel::Mistral(mistral::Client::from_env().completion_model(model_name))
            }
            #[cfg(feature = "moonshot")]
            "moonshot" => {
                AnyModel::Moonshot(moonshot::Client::from_env().completion_model(model_name))
            }
            #[cfg(feature = "ollama")]
            "ollama" => AnyModel::Ollama(ollama::Client::from_env().completion_model(model_name)),
            #[cfg(feature = "openai")]
            "openai" => AnyModel::OpenAi(
                openai::Client::from_env()
                    .completions_api()
                    .completion_model(model_name),
            ),
            #[cfg(feature = "openrouter")]
            "openrouter" => {
                AnyModel::OpenRouter(openrouter::Client::from_env().completion_model(model_name))
            }
            #[cfg(feature = "perplexity")]
            "perplexity" => {
                AnyModel::Perplexity(perplexity::Client::from_env().completion_model(model_name))
            }
            #[cfg(feature = "together")]
            "together" => {
                AnyModel::Together(together::Client::from_env().completion_model(model_name))
            }
            #[cfg(feature = "xai")]
            "xai" => AnyModel::Xai(xai::Client::from_env().completion_model(model_name)),
            other => panic!(
                "Unsupported or disabled provider: \"{other}\". \
                 Make sure the `{other}` feature is enabled in Cargo.toml."
            ),
        }
    }

    /// Create an [`AnyModel`] from `DEFAULT_PROVIDER` and `DEFAULT_MODEL`
    /// environment variables.
    ///
    /// - `DEFAULT_PROVIDER` defaults to `"openai"` if unset.
    /// - `DEFAULT_MODEL` defaults to `"gpt-5.4"` if unset.
    pub fn from_env() -> Self {
        let provider = env::var("DEFAULT_PROVIDER")
            .unwrap_or_else(|_| "openai".to_string())
            .to_lowercase();
        let model_name = env::var("DEFAULT_MODEL").unwrap_or_else(|_| "gpt-5.4-nano".to_string());
        Self::new(&provider, &model_name)
    }

    /// Convenience: build an [`AgentBuilder`] directly from this model.
    ///
    /// # Example
    /// ```rust,ignore
    /// let agent = AnyModel::new("gemini", "gemini-2.5-flash")
    ///     .agent()
    ///     .preamble("You are a helpful assistant.")
    ///     .build();
    /// ```
    pub fn agent(self) -> AgentBuilder<AnyModel> {
        AgentBuilder::new(self)
    }
}
