//! Turn-scoped actions use local prompt overrides; history keeps the original
//! slash-prefixed user text, while providers receive only its arguments.
use super::context::BuiltContext;
use super::request_context::ContextSpec;
use crate::ai::message::{Block, Role};
use crate::ai::provider::{ChatRequest, ErrorKind, ProviderError};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatCommand {
    Generate,
    Explain,
    Optimize,
    Fix,
}

fn command_error() -> ProviderError {
    ProviderError::new(
        ErrorKind::Other,
        "Invalid AI command. Use /help to see the available commands.",
    )
}

fn parse(text: &str) -> Result<Option<(ChatCommand, &str)>, ProviderError> {
    let Some(text) = text.strip_prefix('/') else {
        return Ok(None);
    };
    let end = text.find(char::is_whitespace).unwrap_or(text.len());
    let command = match &text[..end] {
        "generate" => ChatCommand::Generate,
        "explain" => ChatCommand::Explain,
        "optimize" => ChatCommand::Optimize,
        "fix" => ChatCommand::Fix,
        _ => return Err(command_error()),
    };
    Ok(Some((command, text[end..].trim())))
}

pub(super) fn prepare_request(
    req: &mut ChatRequest,
    context: Option<&ContextSpec>,
) -> Result<(), ProviderError> {
    let expected = context.and_then(|spec| spec.command);
    let text = req
        .messages
        .last_mut()
        .filter(|message| message.role == Role::User)
        .and_then(|message| {
            message.blocks.iter_mut().find_map(|block| match block {
                Block::Text { text } => Some(text),
                _ => None,
            })
        });
    let Some(text) = text else {
        return if expected.is_some() {
            Err(command_error())
        } else {
            Ok(())
        };
    };
    let parsed = parse(text)?;
    if parsed.as_ref().map(|(command, _)| *command) != expected {
        return Err(command_error());
    }
    if let Some((_, args)) = parsed {
        *text = if args.is_empty() {
            "Work on the selected SQL, or the current query when no selection is provided.".into()
        } else {
            args.to_string()
        };
    }
    Ok(())
}

fn append_instruction(
    built: &mut BuiltContext,
    command: ChatCommand,
    raw: &str,
    language: &str,
) -> Result<(), ProviderError> {
    if raw.chars().count() > 16_000 {
        return Err(ProviderError::new(
            ErrorKind::Other,
            "The AI command prompt is too long. Shorten it in Settings.",
        ));
    }
    let language = if !language.is_empty()
        && language.len() <= 32
        && language
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        language
    } else {
        "en"
    };
    let prompt = raw
        .replace(
            "{{SCHEMA}}",
            "Use the database schema supplied in the context above.",
        )
        .replace("{{LANGUAGE}}", language);
    let action = match command {
        ChatCommand::Generate => "Generate SQL for the user's request, using the current query when no request is given.",
        ChatCommand::Explain => "Explain the selected SQL, or the current query if there is no selection.",
        ChatCommand::Optimize => "Propose improvements to the selected SQL, or the current query. Use available index and EXPLAIN metadata; say when a plan is unavailable.",
        ChatCommand::Fix => "Propose a correction to the selected SQL, or the current query, using the supplied error. Say when no error is available.",
    };
    let instruction = format!("\n\nAction for this turn:\n{action}\n{prompt}\nUse fenced sql blocks for every SQL proposal. The mode, read-only, missing-metadata and untrusted-data rules above still apply. Do not execute SQL or claim to have done so.");
    built.base_chars += instruction.chars().count();
    built.system.push_str(&instruction);
    Ok(())
}

pub(super) async fn resolve(
    app: &AppHandle,
    spec: &ContextSpec,
) -> Result<BuiltContext, ProviderError> {
    let mut built = super::request_context::resolve(app, spec).await?;
    if let Some(command) = spec.command {
        let prompt = match command {
            ChatCommand::Generate => crate::config::get_system_prompt(app.clone()),
            ChatCommand::Explain => crate::config::get_explain_prompt(app.clone()),
            ChatCommand::Optimize => crate::config::get_optimize_prompt(app.clone()),
            ChatCommand::Fix => crate::config::get_fix_prompt(app.clone()),
        };
        append_instruction(&mut built, command, &prompt, &spec.language)?;
    }
    Ok(built)
}

#[cfg(test)]
mod tests;
