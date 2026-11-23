// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

use super::{DecideError, ModelDecision};
use eidetic::{Hash, ManifestId};
use esp::infer::chat::{ChatMessage, ChatTemplate};
use esp::infer::provider::{GenerationCheckpoint, SessionIdentity};
use esp::infer::{
    BoundModelSession, GenerationRequest, InferError, InferenceProvider, ModelCapability,
    ModelSession,
};
use serde_json::Value;
use std::ops::ControlFlow;
use std::sync::Arc;

/// Prompt renderer and the exact template whose source a model session binds.
pub struct ChatPrompt {
    template: Arc<ChatTemplate>,
    system: String,
    ask: String,
    variables: Value,
}

/// Select the model's chat template and render switches from its tokenizer
/// configuration. The output grammar independently constrains each decision.
///
/// # Errors
/// Returns an error when the configuration lacks a parseable chat template.
pub fn chat_prompt(
    tokenizer_config: &[u8],
    system: String,
    ask: String,
    variables: Value,
) -> Result<ChatPrompt, DecideError> {
    Ok(ChatPrompt {
        template: Arc::new(ChatTemplate::from_tokenizer_config(tokenizer_config)?),
        system,
        ask,
        variables,
    })
}

impl ChatPrompt {
    /// The end-of-turn token used by the selected template.
    pub fn eos(&self) -> &str {
        self.template.eos()
    }


    /// Render the authorized context with the selected system turn and request.
    ///
    /// # Errors
    /// Returns the template's render or variable validation failure.
    pub fn render(&self, context: &str) -> Result<String, DecideError> {
        let messages = [
            ChatMessage::new("system", self.system.as_str()),
            ChatMessage::new("user", format!("{context}\n{}", self.ask)),
        ];
        Ok(self.template.render(&messages, &self.variables)?)
    }
}

/// Accepts already-rendered prompts. Preparation checks the very same template
/// that rendered the turn. ESP owns session validation and checkpoint identity.
struct DecisionProvider<P> {
    bound: BoundModelSession<P>,
    template: Arc<ChatTemplate>,
}

impl<P: InferenceProvider> InferenceProvider for DecisionProvider<P> {
    fn capability(&self) -> &ModelCapability {
        self.bound.provider().capability()
    }

    fn session_identity(&self) -> Option<SessionIdentity> {
        Some(self.bound.identity())
    }

    fn generate_checkpointed(
        &self,
        request: &GenerationRequest,
        resume: Option<&GenerationCheckpoint>,
        commit: &mut dyn FnMut(&GenerationCheckpoint) -> Result<(), InferError>,
        stop: &mut dyn FnMut() -> bool,
    ) -> Result<GenerationCheckpoint, InferError> {
        let prepared = self
            .bound
            .prepare(request.clone(), self.template.source_bytes())?;
        self.bound
            .generate_prepared_checkpointed(&prepared, resume, commit, stop)
    }

    fn generate_streaming(
        &self,
        request: &GenerationRequest,
        on_token: &mut dyn FnMut(&str) -> ControlFlow<()>,
    ) -> Result<String, InferError> {
        let prepared = self
            .bound
            .prepare(request.clone(), self.template.source_bytes())?;
        self.bound.generate_prepared_streaming(&prepared, on_token)
    }
}

/// A provider bound to the renderer and exact artifacts used for a model decision.
pub struct ModelDecider {
    provider: Box<dyn InferenceProvider>,
    prompt: ChatPrompt,
}

impl ModelDecider {
    /// Bind an adapter-free provider loaded from the given content identities.
    ///
    /// # Errors
    /// Returns an error if the provider cannot be bound to a valid session.
    pub fn bind<P: InferenceProvider + 'static>(
        provider: P,
        base_model_ref: ManifestId,
        tokenizer_ref: ManifestId,
        prompt: ChatPrompt,
    ) -> Result<Self, DecideError> {
        let cap = provider.capability();
        let session = ModelSession {
            base_model_ref,
            model_id: cap.model_id.clone(),
            tokenizer_ref,
            prompt_template_hash: Hash::of(prompt.template.source_bytes()),
            quantization: cap.quantization.clone(),
            loader: cap.loader.clone(),
            adapters: vec![],
        };
        let bound = BoundModelSession::bind(session, provider).map_err(InferError::from)?;
        Self::from_session(bound, prompt)
    }

    /// Attach the renderer to an existing session after checking the exact
    /// template bytes against its declared template hash.
    ///
    /// # Errors
    /// Returns an error if the renderer's template differs from the session.
    pub fn from_session<P: InferenceProvider + 'static>(
        bound: BoundModelSession<P>,
        prompt: ChatPrompt,
    ) -> Result<Self, DecideError> {
        bound
            .prepare(GenerationRequest::default(), prompt.template.source_bytes())
            .map_err(InferError::from)?;
        Ok(Self {
            provider: Box::new(DecisionProvider {
                bound,
                template: Arc::clone(&prompt.template),
            }),
            prompt,
        })
    }

    /// Render the context and make one validated decision.
    ///
    /// # Errors
    /// Returns a render, generation or operation-schema validation error.
    pub fn decide(
        &self,
        context: &str,
        operations: &Value,
        max_tokens: usize,
    ) -> Result<ModelDecision, DecideError> {
        self.decide_until(context, operations, max_tokens, &|| false)
    }

    /// Make one decision with cancellation checked at token boundaries.
    ///
    /// # Errors
    /// As [`Self::decide`]. An interrupted turn returns a failure.
    pub fn decide_until(
        &self,
        context: &str,
        operations: &Value,
        max_tokens: usize,
        stop: &dyn Fn() -> bool,
    ) -> Result<ModelDecision, DecideError> {
        let rendered = self.prompt.render(context)?;
        Ok(super::decide_until(
            self.provider.as_ref(),
            &rendered,
            operations,
            max_tokens,
            stop,
        )?)
    }

    /// Revalidate the recorded tokens under this exact session and operation schema.
    ///
    /// # Errors
    /// Returns an error for a different session, schema or invalid checkpoint.
    pub fn replay(
        &self,
        operations: &Value,
        recorded: &GenerationCheckpoint,
    ) -> Result<ModelDecision, DecideError> {
        Ok(super::replay(self.provider.as_ref(), operations, recorded)?)
    }
}
