// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Resolve exact external package identities from the compiled workspace lock.

use serde::Deserialize;
use serde_json::{Value, json};

type Error = Box<dyn std::error::Error + Send + Sync>;
type Result<T> = std::result::Result<T, Error>;

#[derive(Deserialize)]
struct Lockfile {
    package: Vec<LockPackage>,
}

#[derive(Deserialize)]
struct LockPackage {
    name: String,
    version: String,
    source: Option<String>,
    #[serde(default)]
    dependencies: Vec<String>,
}

/// Return exact selected package identities from the compiled workspace lock.
/// Each requested name must be reachable from `mere-mesh` in the lock graph and resolve
/// to exactly one git-sourced package entry.
pub fn selected_git_packages(lock: &[u8], names: &[&str]) -> Result<Vec<Value>> {
    let lock = std::str::from_utf8(lock)?;
    let lock: Lockfile = toml::from_str(lock)?;

    let roots: Vec<_> = lock
        .package
        .iter()
        .filter(|package| package.name == "mere-mesh")
        .collect();
    if roots.len() != 1 {
        return Err(format!(
            "Cargo.lock must contain exactly one mere-mesh package; found {}",
            roots.len()
        )
        .into());
    }

    // Networking is supplied through Stickleback, so validate the selected
    // dependency closure rather than assuming the network crate is direct.
    let root_index = lock
        .package
        .iter()
        .position(|p| std::ptr::eq(p, roots[0]))
        .unwrap();
    let mut pending = vec![root_index];
    let mut reachable = std::collections::BTreeSet::new();
    while let Some(index) = pending.pop() {
        if !reachable.insert(index) {
            continue;
        }
        for dependency in &lock.package[index].dependencies {
            let fields: Vec<_> = dependency.split_whitespace().collect();
            let name = fields.first().copied().ok_or("empty selected dependency")?;
            let version = fields.get(1).copied();
            let source = fields.get(2).map(|s| s.trim_matches(['(', ')']));
            let candidates: Vec<_> = lock
                .package
                .iter()
                .enumerate()
                .filter(|(_, p)| {
                    p.name == name
                        && version.is_none_or(|v| p.version == v)
                        && source.is_none_or(|v| p.source.as_deref() == Some(v))
                })
                .map(|(i, _)| i)
                .collect();
            if candidates.len() != 1 {
                return Err(
                    format!("ambiguous or absent selected dependency: {dependency}").into(),
                );
            }
            pending.push(candidates[0]);
        }
    }
    let mut selected = Vec::with_capacity(names.len());
    for name in names {
        if !reachable.iter().any(|i| lock.package[*i].name == *name) {
            return Err(
                format!("Cargo.lock package {name} is not reachable from mere-mesh").into(),
            );
        }

        // A direct root selection wins over unrelated versions elsewhere in
        // the workspace. Cargo's dependency string binds version/source when
        // the name alone is ambiguous. Delegated libraries must be unique in
        // the reachable closure.
        let direct = roots[0]
            .dependencies
            .iter()
            .find(|dependency| dependency.split_whitespace().next() == Some(*name));
        let matches: Vec<_> = reachable
            .iter()
            .map(|i| &lock.package[*i])
            .filter(|package| package.name.as_str() == *name)
            .filter(|package| match direct {
                None => true,
                Some(dependency) => {
                    let fields: Vec<_> = dependency.split_whitespace().collect();
                    fields.get(1).is_none_or(|v| package.version == *v)
                        && fields.get(2).is_none_or(|v| {
                            package.source.as_deref() == Some(v.trim_matches(['(', ')']))
                        })
                },
            })
            .collect();
        if matches.len() != 1 {
            return Err(format!(
                "Cargo.lock must contain exactly one selected package named {name}; found {}",
                matches.len()
            )
            .into());
        }
        let package = matches[0];
        let source = package
            .source
            .as_deref()
            .ok_or_else(|| format!("Cargo.lock package {name} has no source"))?;
        if !source.starts_with("git+") {
            return Err(format!("Cargo.lock package {name} is not git sourced: {source}").into());
        }
        let revision = source
            .rsplit_once('#')
            .map(|(_, revision)| revision)
            .filter(|revision| {
                revision.len() == 40 && revision.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
            .ok_or_else(|| format!("Cargo.lock package {name} has no resolved git revision"))?;

        selected.push(json!({
            "name": package.name.as_str(),
            "version": package.version.as_str(),
            "source": source,
            "resolved_revision": revision,
        }));
    }
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::selected_git_packages;

    const DEPENDENCY: &str = "mere-transport";
    const VALID_LOCK: &str = r#"
version = 4

[[package]]
name = "mere-mesh"
version = "0.0.1"
dependencies = ["mere-transport"]

[[package]]
name = "mere-transport"
version = "0.2.0"
source = "git+https://example.test/murm?branch=main#0123456789abcdef0123456789abcdef01234567"
"#;

    #[test]
    fn returns_actual_git_package_revision() {
        let selected = selected_git_packages(VALID_LOCK.as_bytes(), &[DEPENDENCY]).unwrap();
        assert_eq!(selected[0]["name"], DEPENDENCY);
        assert_eq!(selected[0]["version"], "0.2.0");
        assert_eq!(
            selected[0]["resolved_revision"],
            "0123456789abcdef0123456789abcdef01234567"
        );
    }

    #[test]
    fn rejects_missing_package() {
        let lock = VALID_LOCK.replace("name = \"mere-transport\"", "name = \"other\"");
        assert!(selected_git_packages(lock.as_bytes(), &[DEPENDENCY]).is_err());
    }

    #[test]
    fn rejects_ambiguous_package_name() {
        let duplicate = r#"
[[package]]
name = "mere-transport"
version = "0.2.1"
source = "git+https://example.test/murm?branch=main#1123456789abcdef0123456789abcdef01234567"
"#;
        let lock = format!("{VALID_LOCK}\n{duplicate}");
        assert!(selected_git_packages(lock.as_bytes(), &[DEPENDENCY]).is_err());
    }

    #[test]
    fn rejects_non_git_package() {
        let lock = VALID_LOCK.replace(
            "git+https://example.test/murm?branch=main#0123456789abcdef0123456789abcdef01234567",
            "registry+https://example.test/index",
        );
        assert!(selected_git_packages(lock.as_bytes(), &[DEPENDENCY]).is_err());
    }

    #[test]
    fn accepts_transitive_source_only_through_a_selected_parent() {
        let lock = VALID_LOCK.replace(
            "dependencies = [\"mere-transport\"]",
            "dependencies = [\"stickleback\"]",
        ) + "\n[[package]]\nname = \"stickleback\"\nversion = \"0.1.1\"\ndependencies = [\"mere-transport\"]\n";
        assert!(selected_git_packages(lock.as_bytes(), &[DEPENDENCY]).is_ok());
        let disconnected = lock.replace("dependencies = [\"stickleback\"]", "dependencies = []");
        assert!(selected_git_packages(disconnected.as_bytes(), &[DEPENDENCY]).is_err());
    }

    #[test]
    fn rejects_package_not_selected_by_root() {
        let lock = VALID_LOCK.replace("dependencies = [\"mere-transport\"]", "dependencies = []");
        assert!(selected_git_packages(lock.as_bytes(), &[DEPENDENCY]).is_err());
    }

    #[test]
    fn rejects_unresolved_or_malformed_git_revision() {
        for fragment in ["", "main", "0123456789abcdef0123456789abcdef0123456g"] {
            let lock = VALID_LOCK.replace("0123456789abcdef0123456789abcdef01234567", fragment);
            assert!(selected_git_packages(lock.as_bytes(), &[DEPENDENCY]).is_err());
        }
    }

    #[test]
    fn accepts_compiled_workspace_lockfile() {
        let lock = include_bytes!("../../../../Cargo.lock");
        let selected = selected_git_packages(
            lock,
            &[
                "mere-p2panda-net",
                "mere-transport",
                "muniment",
                "stickleback",
            ],
        )
        .unwrap();
        assert_eq!(selected.len(), 4);
    }
}
