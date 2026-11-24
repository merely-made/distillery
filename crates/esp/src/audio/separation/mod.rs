// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Music source separation with BS-RoFormer, ported from Watershed on its
//! plain-Burn path (see `NOTICE.md` here for provenance).
//!
//! A stereo song goes in at the checkpoint's sample rate; one stereo buffer
//! per stem comes out, in the order the checkpoint's config lists them. The
//! caller chooses the [`Device`](burn::prelude::Device) and fetches the
//! checkpoint and its `.yaml`; ESP bundles no weights.

mod chunks;
mod config;
mod layers;
mod model;
mod plain;
mod separate;
mod stft;
mod weights;

pub use chunks::{Chunking, overlap_add};
pub use config::{Config, ConfigError};
pub use model::{BsRoformer, Precision};
pub use realfft::num_complex::Complex32;
pub use separate::Cancelled;
pub use stft::Stft;
pub use weights::{Weights, WeightsError, convert_to_half};

/// Stereo audio with interleaved `[left, right, left, right, ...]` samples.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StereoBuffer {
    pub sample_rate: u32,
    pub samples: Vec<f32>,
}

impl StereoBuffer {
    pub fn new(sample_rate: u32, samples: Vec<f32>) -> Self {
        debug_assert!(
            samples.len().is_multiple_of(2),
            "stereo buffer needs an even sample count"
        );
        Self {
            sample_rate,
            samples,
        }
    }

    /// Number of frames (one left and one right sample each).
    pub fn frames(&self) -> usize {
        self.samples.len() / 2
    }

    pub fn duration_secs(&self) -> f64 {
        self.frames() as f64 / f64::from(self.sample_rate)
    }
}
