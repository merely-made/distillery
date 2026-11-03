# Copyright 2026 Mark Alan Boykin
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.
# SPDX-License-Identifier: MPL-2.0

# Build the probe for wasm32 and run it under Node.
#   rustup target add wasm32-unknown-unknown   (once)
$ErrorActionPreference = "Stop"
Push-Location $PSScriptRoot
try {
    $env:CARGO_TARGET_DIR = 'C:\t\cargo-targets\esp'
    $env:CARGO_BUILD_BUILD_DIR = $env:CARGO_TARGET_DIR
    cargo build -p mesh-lexical-wasm --release --locked --target wasm32-unknown-unknown
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    node run.mjs (Join-Path $env:CARGO_TARGET_DIR 'wasm32-unknown-unknown\release\mesh_lexical_wasm.wasm')
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
} finally {
    Pop-Location
}
