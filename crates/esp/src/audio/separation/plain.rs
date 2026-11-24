// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Plain Burn in place of Watershed's own GPU kernels.
//!
//! Watershed fuses attention and hand-picks CubeCL matmul strategies. Those
//! were left out of this port on purpose (Mere's Burn ecosystem survey brief,
//! 2026-11-22): they bind CubeCL internals, and their thresholds were tuned
//! on two machines. This module keeps the names the layers call, so the
//! kernels can return behind it once the plain path is measured. Every
//! accelerated entry point here declines, which sends the layers down the
//! fallback Watershed already had and tested.

use burn::prelude::Tensor;

/// The most attention scores computed at once: 128 MiB in f16. The time
/// layers' attention is sliced to stay under it, which is what bounds GPU
/// memory, so the slicing stays even without the kernels.
pub(crate) const MAX_SCORES: usize = 1 << 26;

/// Watershed's matmul strategy choice. Plain Burn picks its own.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Kernel;

impl Kernel {
    pub(crate) fn for_shape(_rows: usize, _inner: usize, _columns: usize) -> Self {
        Self
    }

    pub(crate) fn for_attention(_sequence: usize) -> Self {
        Self
    }
}

/// `lhs @ rhs` through Burn.
pub(crate) fn matmul<const D: usize>(lhs: Tensor<D>, rhs: Tensor<D>, _kernel: Kernel) -> Tensor<D> {
    lhs.matmul(rhs)
}

/// No fused softmax: the caller falls back to Burn's.
pub(crate) fn softmax<const D: usize>(_x: &Tensor<D>) -> Option<Tensor<D>> {
    None
}

/// No rotary tables: without them the fused attention is never attempted.
pub(crate) struct RotaryTables;

impl RotaryTables {
    pub(crate) fn new(_cos: &Tensor<1>, _sin: &Tensor<1>) -> Option<Self> {
        None
    }
}

/// No fused attention: the caller computes it with Burn ops.
pub(crate) fn attention(
    _qkv: &Tensor<3>,
    _gates: &Tensor<3>,
    _rotary: &RotaryTables,
    _heads: usize,
) -> Option<Tensor<3>> {
    None
}
