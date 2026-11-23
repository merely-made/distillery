// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! A decision driver's turn inside the run loop (decision-driver P2, D12,
//! D13). The worker body picks the driver for the proposing step and
//! `evaluate` for the rest; the journal keeps what each turn reserved, which
//! driver answered with how many tokens, and why a turn failed.

use super::*;
use crate::alembic::workshop::ModelTurn;
use crate::decision::{Decision, DecisionDriver, DecisionSession};

/// Bounds on what a journal entry keeps of a driver's own words.
pub(super) const MAX_PROVIDER: usize = 1024;
pub(super) const MAX_REASON: usize = 512;

/// A decision driver for the proposing step and the token cap each of its
/// turns is given and reserves (decision-driver D12).
#[derive(Clone)]
pub struct Decider {
    pub driver: Arc<dyn DecisionDriver>,
    pub max_tokens: usize,
}

impl Decider {
    pub fn new(driver: Arc<dyn DecisionDriver>, max_tokens: usize) -> Self {
        Self { driver, max_tokens }
    }
    /// What the proposing step's intent reserves.
    pub fn reservation(&self) -> u64 {
        u64::try_from(self.max_tokens).unwrap_or(u64::MAX)
    }
}

impl std::fmt::Debug for Decider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Decider")
            .field("max_tokens", &self.max_tokens)
            .finish_non_exhaustive()
    }
}

/// The driver behind a recorded proposing result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ModelUse {
    pub(super) provider: String,
    pub(super) tokens: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) session: Option<DecisionSession>,
}

impl ModelUse {
    pub(super) fn of(decision: &Decision) -> Self {
        Self {
            provider: bounded(&decision.provider, MAX_PROVIDER),
            tokens: decision.tokens,
            session: decision.session.clone(),
        }
    }
    /// Only the proposing step has a driver, and it may not use more than
    /// its intent reserved.
    pub(super) fn fits(&self, step: u64, reserved: u64) -> bool {
        let valid_identity = |value: &str| !value.is_empty() && value.len() <= MAX_PROVIDER;
        step == 1
            && self.tokens <= reserved
            && self.provider.len() <= MAX_PROVIDER
            && self.session.as_ref().is_none_or(|s| {
                valid_identity(&s.session_id) && valid_identity(&s.prompt_template_hash)
            })
    }
}

/// What a driver contributed to an observation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Outcome {
    Decided(ModelUse),
    /// The turn failed, or its decision was refused; the reason is bounded.
    Failed(String),
}

fn bounded(text: &str, limit: usize) -> String {
    let mut end = text.len().min(limit);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].into()
}

#[cfg(all(feature = "workshop-worker", not(target_arch = "wasm32")))]
impl Observation {
    fn failed(correlation: Correlation, error: &crate::decision::DecisionError) -> Self {
        Self {
            correlation,
            proposal: None,
            turn: Some(Outcome::Failed(bounded(&format!("{error:?}"), MAX_REASON))),
        }
    }
}

/// The worker body. A driver's failure or a decision outside the offered
/// choice is an observation to journal (D13); a dispatch the proposing path
/// refuses, such as a private one, is an error and the owner pauses the run.
///
/// # Errors
/// As `evaluate`, and when the proposing path refuses the dispatch itself.
#[cfg(all(feature = "workshop-worker", not(target_arch = "wasm32")))]
pub(super) fn work(
    dispatch: Dispatch,
    decider: Option<&Decider>,
    cancelled: &dyn Fn() -> bool,
) -> Result<Observation, Error> {
    match decider {
        Some(decider) if dispatch.correlation.step == 1 => {
            let c = dispatch.correlation;
            match propose_until(
                decider.driver.as_ref(),
                dispatch,
                decider.max_tokens,
                cancelled,
            ) {
                Ok(proposed) => Ok(proposed.observation),
                Err(ProposeError::Driver(error)) => Ok(Observation::failed(c, &error)),
                Err(ProposeError::Workshop(error)) => Err(error),
            }
        },
        _ => evaluate(dispatch),
    }
}

/// The per-turn token cap a run's limits were made from: `None` for the
/// model-free [`LIMITS`], the cap for [`model_limits`] (D15).
///
/// # Errors
/// When the limits are neither profile.
pub(super) fn cap(limits: &Limits) -> Result<Option<u64>, Error> {
    if *limits == LIMITS {
        return Ok(None);
    }
    let cap = limits.tokens / 2;
    if model_limits(cap).as_ref() == Some(limits) {
        Ok(Some(cap))
    } else {
        Err(Error::Invalid("run limits"))
    }
}

/// What an intent at `step` reserves: the cap at a model run's proposing
/// step, nothing anywhere else.
pub(super) fn reservation(cap: Option<u64>, step: u64) -> u64 {
    if step == 1 { cap.unwrap_or(0) } else { 0 }
}

/// A journalled failed turn as the reducer reads it (D13). One fewer retry
/// than the failure limit: with the limit at two, the first failure retries
/// and the second fails the run.
pub(super) fn failed_event(
    reducer: &RunReducer,
    limits: &Limits,
    c: Correlation,
    at: u64,
    reason: &str,
) -> Result<RunEvent, Error> {
    if c.step != 1 || reason.len() > MAX_REASON {
        return Err(Error::Invalid("failed turn"));
    }
    let retry = reducer
        .usage()
        .consecutive_failures
        .checked_add(1)
        .is_some_and(|failures| failures < limits.consecutive_failures);
    Ok(RunEvent::ResultRecorded {
        correlation: correlation(c),
        result: if retry {
            servitor::ResultKind::RetryableFailure
        } else {
            servitor::ResultKind::Failed
        },
        usage: servitor::Usage {
            elapsed_ms: at.checked_sub(reducer.last_at_ms()).ok_or(Error::Stale)?,
            consecutive_failures: 1,
            ..Default::default()
        },
        at_ms: at,
    })
}

/// The run's driver turns as the view shows them.
pub(super) fn turns(run: &Run) -> Vec<ModelTurn> {
    let mut reserved = 0;
    let mut turns = vec![];
    for entry in &run.entries {
        match entry {
            Entry::Intent { reserved: r, .. } => reserved = *r,
            Entry::Result {
                correlation,
                model: Some(model),
                ..
            } => turns.push(ModelTurn {
                correlation: *correlation,
                reserved,
                provider: Some(model.provider.clone()),
                session: model.session.clone(),
                tokens: model.tokens,
                failure: None,
            }),
            Entry::Failed {
                correlation,
                reason,
                ..
            } => turns.push(ModelTurn {
                correlation: *correlation,
                reserved,
                provider: None,
                session: None,
                tokens: 0,
                failure: Some(reason.clone()),
            }),
            _ => {},
        }
    }
    turns
}
