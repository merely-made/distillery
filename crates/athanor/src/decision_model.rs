// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! A model as the decision driver (decision-driver D10). Distillery owns the
//! model turn and knows nothing of this crate; this is the whole of the glue.

use crate::decision::{Decision, DecisionDriver, DecisionError, DecisionSession, Turn};
use distillery::decision::{DecideError, ModelDecider};

impl DecisionDriver for ModelDecider {
    fn decide(&self, turn: &Turn<'_>) -> Result<Decision, DecisionError> {
        // The turn's cancellation is esp's stop: generation pauses at a token
        // boundary and the turn fails instead of returning a fragment.
        match self.decide_until(
            turn.context,
            turn.operations,
            turn.max_tokens,
            turn.cancelled,
        ) {
            Ok(decision) => {
                let session = decision.checkpoint.session.as_ref().ok_or_else(|| {
                    DecisionError::Failed("model decision lacks its session".into())
                })?;
                Ok(Decision {
                    value: decision.value,
                    provider: decision.provider,
                    session: Some(DecisionSession {
                        session_id: session.session_id.clone(),
                        prompt_template_hash: session.prompt_template_hash.clone(),
                    }),
                    tokens: decision.tokens,
                })
            },
            Err(DecideError::Unsupported(detail)) => Err(DecisionError::Unsupported(detail)),
            Err(DecideError::Invalid(detail)) => Err(DecisionError::Invalid(detail)),
            Err(DecideError::Failed(detail)) => Err(DecisionError::Failed(detail)),
        }
    }
}
