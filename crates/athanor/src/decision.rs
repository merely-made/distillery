// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! One decision turn (agent harness brief, section 2; decision-driver D5, D9).
//!
//! A driver is a script, a deterministic algorithm or a model. It is handed
//! context the caller was authorized to read and the operations it may choose
//! among, as one JSON Schema, and returns one decision. It holds no authority
//! and causes no effect: the caller checks the decision against its own domain
//! and rechecks live authority before acting on it.

use serde_json::Value;
use std::collections::VecDeque;
use std::sync::Mutex;

/// What a driver is asked. `operations` is a JSON Schema; every decision the
/// driver returns must satisfy it.
#[derive(Clone, Copy)]
pub struct Turn<'a> {
    pub context: &'a str,
    pub operations: &'a Value,
    /// Hard cap on what the driver may generate.
    pub max_tokens: usize,
    /// True once the caller has cancelled the turn. A driver that generates
    /// checks it at each token boundary and then fails; it never returns a
    /// fragment as a decision.
    pub cancelled: &'a dyn Fn() -> bool,
}

impl std::fmt::Debug for Turn<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Turn")
            .field("context", &self.context)
            .field("operations", &self.operations)
            .field("max_tokens", &self.max_tokens)
            .field("cancelled", &(self.cancelled)())
            .finish()
    }
}

/// Immutable model-session and chat-template content identities. The optional
/// workshop feature journals these fields without depending on ESP.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "workshop", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "workshop", serde(deny_unknown_fields))]
pub struct DecisionSession {
    pub session_id: String,
    pub prompt_template_hash: String,
}

/// One decision, with what the caller retains about how it was made.
#[derive(Clone, Debug, PartialEq)]
pub struct Decision {
    pub value: Value,
    /// Provider and loader, as the driver names itself.
    pub provider: String,
    /// Exact binding for a model-backed decision. Scripted drivers omit it.
    pub session: Option<DecisionSession>,
    /// Observed generation usage; zero for a driver that generates nothing.
    pub tokens: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecisionError {
    /// The driver cannot serve these operations or limits.
    Unsupported(String),
    /// A decision outside the operations it was offered.
    Invalid(String),
    /// The driver ran and did not produce a decision.
    Failed(String),
}

impl std::fmt::Display for DecisionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "decision: {self:?}")
    }
}
impl std::error::Error for DecisionError {}

pub trait DecisionDriver: Send + Sync {
    /// # Errors
    /// When the driver cannot serve the turn or produces no valid decision.
    fn decide(&self, turn: &Turn<'_>) -> Result<Decision, DecisionError>;
}

/// A fixed script of decisions, handed out in order: the deterministic driver
/// for scenarios and tests. It does not read the turn.
#[derive(Debug, Default)]
pub struct Scripted(Mutex<VecDeque<Value>>);

impl Scripted {
    pub fn new(decisions: impl IntoIterator<Item = Value>) -> Self {
        Self(Mutex::new(decisions.into_iter().collect()))
    }
}

impl DecisionDriver for Scripted {
    fn decide(&self, _: &Turn<'_>) -> Result<Decision, DecisionError> {
        let next = self
            .0
            .lock()
            .map_err(|_| DecisionError::Failed("script lock poisoned".into()))?
            .pop_front();
        next.map(|value| Decision {
            value,
            provider: "athanor.scripted".into(),
            session: None,
            tokens: 0,
        })
        .ok_or_else(|| DecisionError::Failed("script exhausted".into()))
    }
}
