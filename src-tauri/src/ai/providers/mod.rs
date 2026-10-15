mod anthropic;
mod capabilities;
mod decoder;
mod history;
mod ollama;
mod openai_compat;
mod transport;

pub use capabilities::*;
pub use transport::{configured_provider, HttpProvider};

#[cfg(test)]
pub(super) fn legacy_anthropic_body(
    req: &crate::ai::provider::ChatRequest,
) -> (serde_json::Value, Vec<crate::ai::provider::ChatEvent>) {
    anthropic::request(
        req,
        crate::ai::catalog::capabilities("anthropic", &req.model),
    )
}

#[cfg(test)]
mod tests;
