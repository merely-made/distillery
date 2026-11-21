// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! A decision driver at the workshop's proposing step (decision-driver D9,
//! D11). The driver chooses which of the owner's selected passages a brief
//! cites and in what order. The brief is still built by `proposal`, so its
//! text, citations and integrity checks are the deterministic ones. Steps 0
//! and 2 stay with `evaluate`. Private inputs are never shown to a driver.

use super::*;
use crate::decision::{Decision, DecisionDriver, DecisionError, Turn};
use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq)]
pub enum ProposeError {
    Workshop(Error),
    Driver(DecisionError),
}
impl std::fmt::Display for ProposeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "evidence brief proposal: {self:?}")
    }
}
impl std::error::Error for ProposeError {}

/// A driver-made observation and the decision behind it, for the host to
/// retain (provider identity, usage).
#[derive(Clone, Debug)]
pub struct Proposed {
    pub observation: Observation,
    pub decision: Decision,
}

/// The one operation offered at the proposing step, as JSON Schema: cite at
/// least two of the dispatch's passages, by position, in a chosen order.
pub fn operations(dispatch: &Dispatch) -> Value {
    let count = dispatch.inputs.len();
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["op", "order"],
        "properties": {
            "op": {"const": "brief"},
            "order": {
                "type": "array",
                "minItems": 2,
                "maxItems": count,
                "items": {"type": "integer", "minimum": 0, "maximum": count.saturating_sub(1)},
            },
        },
    })
}

/// The passages as a driver reads them, numbered by position.
pub fn context(dispatch: &Dispatch) -> String {
    dispatch
        .inputs
        .iter()
        .enumerate()
        .map(|(at, passage)| format!("Passage {at}: {}\n{}\n", passage.source.title, passage.text))
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Brief {
    op: String,
    order: Vec<usize>,
}

/// Run `driver` for a step 1 dispatch of public passages.
///
/// # Errors
/// When the dispatch is not a public step 1, the driver fails, or its
/// decision is not a duplicate-free choice of at least two passages.
pub fn propose_with(
    driver: &dyn DecisionDriver,
    dispatch: Dispatch,
    max_tokens: usize,
) -> Result<Proposed, ProposeError> {
    let refuse = |reason| ProposeError::Workshop(Error::Invalid(reason));
    if dispatch.correlation.step != 1 {
        return Err(refuse("workflow step"));
    }
    if dispatch.protected {
        return Err(refuse("private storage is not qualified"));
    }
    let inputs = &dispatch.inputs;
    let owner = &inputs.first().ok_or_else(|| refuse("selection"))?.source.owner;
    validate(owner, inputs).map_err(ProposeError::Workshop)?;
    let operations = operations(&dispatch);
    let decision = driver
        .decide(&Turn {
            context: &context(&dispatch),
            operations: &operations,
            max_tokens,
        })
        .map_err(ProposeError::Driver)?;
    let outside = |detail: &str| ProposeError::Driver(DecisionError::Invalid(detail.into()));
    let brief: Brief = serde_json::from_value(decision.value.clone())
        .map_err(|error| outside(&error.to_string()))?;
    let order = &brief.order;
    if brief.op != "brief"
        || !(2..=inputs.len()).contains(&order.len())
        || order
            .iter()
            .enumerate()
            .any(|(at, index)| *index >= inputs.len() || order[..at].contains(index))
    {
        return Err(outside("not a duplicate-free choice of at least two passages"));
    }
    let chosen: Vec<Passage> = order.iter().map(|index| inputs[*index].clone()).collect();
    let proposal = proposal(dispatch.installation, dispatch.correlation.run, &chosen)
        .map_err(ProposeError::Workshop)?;
    Ok(Proposed {
        observation: Observation {
            correlation: dispatch.correlation,
            proposal: Some(proposal),
        },
        decision,
    })
}
