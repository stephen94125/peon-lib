//! LLM provider implementations.
//!
//! Each provider translates between `peon-runtime`'s unified message types
//! and the provider-specific wire format. All providers implement
//! [`CompletionProvider`](crate::CompletionProvider).
//!
//! ## Available Providers
//!
//! | Provider | Module | API Format |
//! |---|---|---|
//! | OpenAI | [`openai`] | OpenAI Chat Completions |
//! | OpenRouter | [`openai`] (use `OpenAiProvider::openrouter()`) | OpenAI-compatible |
//! | Anthropic | [`anthropic`] | Anthropic Messages |
//! | Gemini | [`gemini`] | Google GenerateContent |

pub mod openai;
pub mod anthropic;
pub mod gemini;
