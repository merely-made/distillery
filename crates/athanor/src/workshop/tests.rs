// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;
use muniment::{MemoryBackend, StoreError, TransactFn};
use servitor::{Grant, GrantTable, Subject};
use std::sync::Barrier;

const SUBJECT: [u8; 32] = [7; 32];
const HUMAN: [u8; 32] = [8; 32];
const ID: [u8; 16] = [3; 16];

fn owner(host: &str) -> Owner {
    Owner {
        host: host.into(),
        workspace: "public-fixture".into(),
        session: "session-1".into(),
    }
}
fn passages(owner: &Owner) -> Vec<Passage> {
    [
        (
            "A",
            "1",
            "Field notes",
            "A retained copy remains readable when its origin is offline.",
        ),
        (
            "B",
            "2",
            "Field notes",
            "The café note cites version 2 of its source.",
        ),
        (
            "C",
            "1",
            "Local research note",
            "Acceptance finishes after the destination confirms storage.",
        ),
    ]
    .into_iter()
    .map(|(resource, version, title, text)| {
        capture(
            Source {
                owner: owner.clone(),
                resource: resource.into(),
                version: version.into(),
                content_digest: [0; 32],
                start: 0,
                end: text.len() as u64,
                title: title.into(),
            },
            text,
            true,
        )
        .unwrap()
    })
    .collect()
}
fn destination(owner: &Owner) -> Destination {
    Destination {
        owner: owner.clone(),
        graph: "research".into(),
        revision: "revision-1".into(),
    }
}
fn authority(owner: &Owner, id: [u8; 16], passages: &[Passage]) -> GrantTable {
    let mut table = GrantTable::new();
    for (cap, mode) in requirements(owner, id, passages) {
        table = table.with_grant(Grant::new(Subject(SUBJECT), cap, mode));
    }
    table.with_grant(Grant::new(
        Subject(HUMAN),
        destination_cap(&destination(owner)),
        Mode::Write,
    ))
}
fn fixture() -> (
    Workshop<MemoryBackend>,
    MemoryBackend,
    GrantTable,
    Vec<Passage>,
) {
    let owner = owner("fixture-one");
    let inputs = passages(&owner);
    let authority = authority(&owner, ID, &inputs);
    let backend = MemoryBackend::new();
    (
        Workshop::new(backend.clone(), owner, ID),
        backend,
        authority,
        inputs,
    )
}
async fn generate<B: WorkshopBackend>(shop: &Workshop<B>, authority: &GrantTable, mut now: u64) {
    for _ in 0..3 {
        let dispatch = shop.dispatch(1, authority, now).await.unwrap();
        now += 1;
        shop.observe(evaluate(dispatch).unwrap(), authority, now)
            .await
            .unwrap();
        now += 1;
    }
}

#[test]
fn exact_order_unicode_and_versioned_sources_survive_reopen() {
    pollster::block_on(async {
        let (shop, backend, auth, inputs) = fixture();
        shop.install(SUBJECT).await.unwrap();
        shop.begin(1, inputs.clone(), &auth, 10).await.unwrap();
        generate(&shop, &auth, 11).await;
        let reopened = Workshop::new(backend, shop.owner.clone(), ID);
        let view = reopened.view(Some(1)).await.unwrap();
        assert_eq!(view.review, Some(Review::AwaitingReview));
        assert_eq!(
            (
                view.usage.decisions,
                view.usage.tool_calls,
                view.usage.tokens
            ),
            (3, 2, 0)
        );
        assert_eq!(view.phase, RunStatus::Completed);
        let brief = view.proposal.unwrap();
        assert_eq!(brief.sources, inputs);
        assert_eq!(brief.sources[1].source.version, "2");
        assert!(brief.djot.starts_with("# Evidence brief\n\n## 1. Field notes\n\n> ```\n> A retained copy remains readable when its origin is offline.\n> ```\n"));
        let a = brief.djot.find("A retained copy").unwrap();
        let b = brief.djot.find("The café note").unwrap();
        let c = brief.djot.find("Acceptance finishes").unwrap();
        assert!(a < b && b < c);
        assert_ne!(
            digest(&brief.sources[0].source),
            digest(&brief.sources[1].source)
        );
        assert_eq!(brief.djot.matches("alembic-source:").count(), 3);
        assert_eq!(
            brief.digest,
            proposal(ID, 1, &brief.sources).unwrap().digest
        );
    });
}

#[test]
fn unicode_selectors_bounds_privacy_and_integrity_refuse() {
    let source = passages(&owner("fixture"))[1].source.clone();
    let text = "The café note cites version 2 of its source.";
    let mut broken = source.clone();
    broken.start = (text.find('é').unwrap() + 1) as u64;
    assert_eq!(
        capture(broken, text, true),
        Err(Error::Invalid("UTF-8 selector"))
    );
    assert!(capture(source.clone(), text, false).is_err());
    let mut large = source;
    large.end = 2049;
    assert!(capture(large, &"x".repeat(2049), true).is_err());
    let mut inputs = passages(&owner("fixture"));
    inputs[0].text.push('!');
    assert!(validate(&owner("fixture"), &inputs).is_err());
    inputs[0].text.pop();
    inputs[0].source.owner.session = "other".into();
    assert!(validate(&owner("fixture"), &inputs).is_err());
    assert!(validate(&owner("fixture"), &[]).is_err());
}

#[test]
fn hostile_djot_is_literal_and_citations_keep_identity() {
    let owner = owner("fixture");
    let mut inputs = passages(&owner);
    let text = "```\n[acquire](https://example.invalid)\n```\n> # inject";
    inputs[0] = capture(
        Source {
            end: text.len() as u64,
            title: "[title](route)".into(),
            ..inputs[0].source.clone()
        },
        text,
        true,
    )
    .unwrap();
    let brief = proposal(ID, 1, &inputs).unwrap();
    assert!(brief.djot.contains("## 1. \\[title\\]\\(route\\)"));
    assert!(
        brief
            .djot
            .contains("> ````\n> ```\n> [acquire](https://example.invalid)")
    );
    assert!(brief.djot.len() <= 16384);
}

#[test]
fn citation_routes_to_the_captured_version_after_the_live_source_changes() {
    let mut live = passages(&owner("fixture"));
    let brief = proposal(ID, 1, &live).unwrap();
    let route = format!("alembic-source:{}", hex(&digest(&live[1].source)));
    live[1].source.version = "3".into();
    live[1].text = "Changed live bytes".into();
    let source = citation_source(&brief, &route).unwrap();
    assert_eq!(
        (source.resource.as_str(), source.version.as_str()),
        ("B", "2")
    );
    assert_ne!(source.version, live[1].source.version);
    assert!(citation_source(&brief, "https://example.invalid/latest").is_err());
    assert!(citation_source(&brief, &format!("alembic-source:{}", "0".repeat(64))).is_err());
}

#[test]
fn a_fourth_ungranted_source_never_enters_context() {
    pollster::block_on(async {
        let (shop, _, auth, mut inputs) = fixture();
        shop.install(SUBJECT).await.unwrap();
        let mut fourth = inputs[0].clone();
        fourth.source.resource = "outside-grant".into();
        inputs.push(fourth);
        assert_eq!(
            shop.begin(1, inputs, &auth, 0).await,
            Err(Error::Unauthorized)
        );
        assert_eq!(shop.view(None).await.unwrap().run, None);
    });
}

#[test]
fn one_active_run_is_atomic_under_concurrent_invocations() {
    let (shop, backend, auth, inputs) = fixture();
    pollster::block_on(shop.install(SUBJECT)).unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let threads: Vec<_> = (1..=2)
        .map(|id| {
            let shop = Workshop::new(backend.clone(), shop.owner.clone(), ID);
            let barrier = barrier.clone();
            let auth = auth.clone();
            let inputs = inputs.clone();
            std::thread::spawn(move || {
                barrier.wait();
                pollster::block_on(shop.begin(id, inputs, &auth, 0))
            })
        })
        .collect();
    assert_eq!(
        threads
            .into_iter()
            .map(|t| t.join().unwrap())
            .filter(Result::is_ok)
            .count(),
        1
    );
    assert_eq!(pollster::block_on(shop.load()).unwrap().1.runs.len(), 1);
}

#[test]
fn duplicate_and_wrong_generation_observations_do_not_charge_twice() {
    pollster::block_on(async {
        let (shop, _, auth, inputs) = fixture();
        shop.install(SUBJECT).await.unwrap();
        shop.begin(1, inputs, &auth, 0).await.unwrap();
        let observation = evaluate(shop.dispatch(1, &auth, 1).await.unwrap()).unwrap();
        let mut wrong = observation.clone();
        wrong.correlation.generation += 1;
        assert_eq!(shop.observe(wrong, &auth, 2).await, Err(Error::Stale));
        shop.observe(observation.clone(), &auth, 2).await.unwrap();
        assert_eq!(shop.observe(observation, &auth, 3).await, Err(Error::Stale));
        assert_eq!(shop.view(Some(1)).await.unwrap().usage.decisions, 1);
    });
}

#[test]
fn revocation_during_a_worker_await_refuses_the_result() {
    pollster::block_on(async {
        let (shop, _, auth, inputs) = fixture();
        shop.install(SUBJECT).await.unwrap();
        shop.begin(1, inputs, &auth, 0).await.unwrap();
        let observation = evaluate(shop.dispatch(1, &auth, 1).await.unwrap()).unwrap();
        assert_eq!(
            shop.observe(observation.clone(), &GrantTable::new(), 2)
                .await,
            Err(Error::Unauthorized)
        );
        shop.lifecycle(Control::Revoke, &auth, 2).await.unwrap();
        assert_eq!(
            shop.observe(observation, &auth, 3).await,
            Err(Error::Unauthorized)
        );
        let view = shop.view(Some(1)).await.unwrap();
        assert_eq!(view.generation, 2);
        assert_eq!(view.phase, RunStatus::Cancelled);
        assert!(view.proposal.is_none());
    });
}

#[test]
fn pause_is_durable_and_resume_rechecks_current_grants() {
    pollster::block_on(async {
        let (shop, backend, auth, inputs) = fixture();
        shop.install(SUBJECT).await.unwrap();
        shop.begin(1, inputs, &auth, 0).await.unwrap();
        shop.lifecycle(Control::Pause, &auth, 1).await.unwrap();
        let reopened = Workshop::new(backend, shop.owner.clone(), ID);
        assert_eq!(
            reopened.view(Some(1)).await.unwrap().lifecycle,
            InstallationStatus::Paused
        );
        assert_eq!(
            reopened
                .lifecycle(Control::Resume, &GrantTable::new(), 2)
                .await,
            Err(Error::Unauthorized)
        );
        reopened.lifecycle(Control::Resume, &auth, 2).await.unwrap();
        generate(&reopened, &auth, 3).await;
        assert_eq!(
            reopened.view(Some(1)).await.unwrap().review,
            Some(Review::AwaitingReview)
        );
    });
}

#[test]
fn interrupted_replay_never_reexecutes_and_keeps_original_budget() {
    pollster::block_on(async {
        let (shop, backend, auth, inputs) = fixture();
        shop.install(SUBJECT).await.unwrap();
        shop.begin(1, inputs, &auth, 0).await.unwrap();
        let delayed = evaluate(shop.dispatch(1, &auth, 1).await.unwrap()).unwrap();
        let reopened = Workshop::new(backend, shop.owner.clone(), ID);
        assert!(matches!(
            reopened.view(Some(1)).await.unwrap().phase,
            RunStatus::Awaiting(_)
        ));
        reopened.interrupt(1, 2).await.unwrap();
        assert_eq!(reopened.observe(delayed, &auth, 3).await, Err(Error::Stale));
        let retry = reopened.dispatch(1, &auth, 3).await.unwrap();
        assert_eq!(retry.correlation.attempt, 1);
        reopened
            .observe(evaluate(retry).unwrap(), &auth, 4)
            .await
            .unwrap();
        reopened
            .observe(
                evaluate(reopened.dispatch(1, &auth, 5).await.unwrap()).unwrap(),
                &auth,
                6,
            )
            .await
            .unwrap();
        assert_eq!(
            reopened.view(Some(1)).await.unwrap().phase,
            RunStatus::BudgetExhausted
        );
        assert!(reopened.view(Some(1)).await.unwrap().proposal.is_none());
    });
}

#[test]
fn elapsed_budget_includes_the_worker_await() {
    pollster::block_on(async {
        let (shop, _, auth, inputs) = fixture();
        shop.install(SUBJECT).await.unwrap();
        shop.begin(1, inputs, &auth, 0).await.unwrap();
        let result = evaluate(shop.dispatch(1, &auth, 1).await.unwrap()).unwrap();
        assert_eq!(
            shop.observe(result, &auth, 30_000).await,
            Err(Error::Budget)
        );
        let view = shop.view(Some(1)).await.unwrap();
        assert_eq!(view.usage.elapsed_ms, 30_000);
        assert_eq!(view.phase, RunStatus::BudgetExhausted);
    });
}

#[test]
fn concurrent_acceptance_assigns_one_operation_and_keeps_generation_terminal() {
    let (shop, backend, auth, inputs) = fixture();
    pollster::block_on(async {
        shop.install(SUBJECT).await.unwrap();
        shop.begin(1, inputs, &auth, 0).await.unwrap();
        generate(&shop, &auth, 1).await;
    });
    let before = pollster::block_on(shop.view(Some(1))).unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let threads: Vec<_> = (0..2)
        .map(|_| {
            let shop = Workshop::new(backend.clone(), shop.owner.clone(), ID);
            let auth = auth.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                pollster::block_on(shop.accept(1, destination(&shop.owner), HUMAN, &auth, 8))
            })
        })
        .collect();
    let attempts: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert!(attempts.iter().any(Result::is_ok));
    let assigned =
        pollster::block_on(shop.accept(1, destination(&shop.owner), HUMAN, &auth, 8)).unwrap();
    for a in attempts.into_iter().flatten() {
        assert_eq!(a, assigned);
    }
    pollster::block_on(async {
        shop.uncertain(1).await.unwrap();
        let reopened = Workshop::new(backend, shop.owner.clone(), ID);
        assert_eq!(
            reopened
                .accept(1, destination(&shop.owner), HUMAN, &auth, 8)
                .await
                .unwrap(),
            assigned
        );
        let mut other = destination(&shop.owner);
        other.revision = "changed-destination".into();
        assert_eq!(
            reopened.accept(1, other, HUMAN, &auth, 8).await,
            Err(Error::Review)
        );
        let receipt = Receipt {
            operation: assigned.operation,
            proposal: assigned.proposal,
            destination: assigned.destination.clone(),
            resource: "synthetic-note-only".into(),
            storage_witness: "synthetic-owner-ledger-1".into(),
        };
        let mut wrong = receipt.clone();
        wrong.proposal[0] ^= 1;
        assert!(reopened.acknowledge(1, wrong).await.is_err());
        reopened.acknowledge(1, receipt.clone()).await.unwrap();
        reopened.acknowledge(1, receipt).await.unwrap();
        let after = reopened.view(Some(1)).await.unwrap();
        assert!(matches!(after.review, Some(Review::Added { .. })));
        assert_eq!(after.phase, before.phase);
        assert_eq!(after.usage, before.usage);
    });
}

#[test]
fn rejected_proposal_cannot_be_accepted_or_replaced_by_a_delayed_result() {
    pollster::block_on(async {
        let (shop, _, auth, inputs) = fixture();
        shop.install(SUBJECT).await.unwrap();
        shop.begin(1, inputs, &auth, 0).await.unwrap();
        generate(&shop, &auth, 1).await;
        let view = shop.view(Some(1)).await.unwrap();
        shop.reject(1).await.unwrap();
        assert_eq!(
            shop.accept(1, destination(&shop.owner), HUMAN, &auth, 8)
                .await,
            Err(Error::Review)
        );
        assert_eq!(
            shop.observe(
                Observation {
                    correlation: Correlation {
                        run: 1,
                        step: 2,
                        attempt: 0,
                        generation: 1
                    },
                    proposal: view.proposal,
                    turn: None,
                },
                &auth,
                9
            )
            .await,
            Err(Error::Stale)
        );
    });
}

#[test]
fn acceptance_rechecks_human_authority_and_deadline_after_awaits() {
    pollster::block_on(async {
        let (shop, _, auth, inputs) = fixture();
        shop.install(SUBJECT).await.unwrap();
        shop.begin(1, inputs, &auth, 0).await.unwrap();
        generate(&shop, &auth, 1).await;
        let intent = shop
            .accept(1, destination(&shop.owner), HUMAN, &auth, 8)
            .await
            .unwrap();
        assert_eq!((intent.requested_at_ms, intent.deadline_at_ms), (8, 30_008));
        revalidate_acceptance(&intent, HUMAN, &auth, 9).unwrap();
        assert_eq!(
            revalidate_acceptance(&intent, HUMAN, &GrantTable::new(), 9),
            Err(Error::Unauthorized)
        );
        assert_eq!(
            revalidate_acceptance(&intent, SUBJECT, &auth, 9),
            Err(Error::Unauthorized)
        );
        assert_eq!(
            revalidate_acceptance(&intent, HUMAN, &auth, 30_008),
            Err(Error::Budget)
        );
        shop.uncertain(1).await.unwrap();
        assert_eq!(
            shop.accept(1, destination(&shop.owner), HUMAN, &auth, 40_000)
                .await
                .unwrap(),
            intent
        );
    });
}

#[test]
fn revoke_then_dissolve_preserves_an_uncertain_acceptance() {
    pollster::block_on(async {
        let (shop, _, auth, inputs) = fixture();
        shop.install(SUBJECT).await.unwrap();
        shop.begin(1, inputs, &auth, 0).await.unwrap();
        generate(&shop, &auth, 1).await;
        shop.accept(1, destination(&shop.owner), HUMAN, &auth, 8)
            .await
            .unwrap();
        shop.uncertain(1).await.unwrap();
        shop.lifecycle(Control::Revoke, &auth, 9).await.unwrap();
        shop.lifecycle(Control::Dissolve, &auth, 10).await.unwrap();
        let view = shop.view(Some(1)).await.unwrap();
        assert_eq!(view.lifecycle, InstallationStatus::Dissolved);
        assert_eq!(view.phase, RunStatus::Completed);
        assert!(matches!(view.review, Some(Review::Reconciling(_))));
        assert!(view.controls.contains(&Control::Reconcile));
        assert!(!view.controls.contains(&Control::Run));
    });
}

#[test]
fn persisted_artifact_corruption_refuses_instead_of_silently_rebuilding() {
    pollster::block_on(async {
        let (shop, backend, auth, inputs) = fixture();
        shop.install(SUBJECT).await.unwrap();
        shop.begin(1, inputs, &auth, 0).await.unwrap();
        generate(&shop, &auth, 1).await;
        let (_, mut record) = shop.load().await.unwrap();
        record.runs[0]
            .proposal
            .as_mut()
            .unwrap()
            .djot
            .push_str("corruption");
        backend
            .put(&shop.key, &encoded(&record).unwrap())
            .await
            .unwrap();
        assert_eq!(
            shop.view(Some(1)).await,
            Err(Error::Invalid("persisted proposal integrity"))
        );
    });
}

#[test]
fn retained_history_bound_refuses_new_work_without_erasing_pending_review() {
    pollster::block_on(async {
        let (shop, _, auth, inputs) = fixture();
        shop.install(SUBJECT).await.unwrap();
        for id in 1..=MAX_RUNS as u64 {
            let now = id * 10;
            shop.begin(id, inputs.clone(), &auth, now).await.unwrap();
            for step in 0..3 {
                let result =
                    evaluate(shop.dispatch(id, &auth, now + 1 + step * 2).await.unwrap()).unwrap();
                shop.observe(result, &auth, now + 2 + step * 2)
                    .await
                    .unwrap();
            }
        }
        assert_eq!(
            shop.begin(100, inputs, &auth, 1000).await,
            Err(Error::Invalid("run retention bound"))
        );
        let intent = shop
            .accept(1, destination(&shop.owner), HUMAN, &auth, 1000)
            .await
            .unwrap();
        shop.uncertain(1).await.unwrap();
        shop.acknowledge(
            1,
            Receipt {
                operation: intent.operation,
                proposal: intent.proposal,
                destination: intent.destination,
                resource: "synthetic-note".into(),
                storage_witness: "synthetic-ledger".into(),
            },
        )
        .await
        .unwrap();
        assert_eq!(shop.load().await.unwrap().1.runs.len(), MAX_RUNS);
        assert!(matches!(
            shop.view(Some(1)).await.unwrap().review,
            Some(Review::Added { .. })
        ));
    });
}

#[derive(Clone, Default)]
struct Fallible {
    inner: MemoryBackend,
    fail: Arc<AtomicBool>,
    nontransactional: bool,
}
#[async_trait::async_trait]
impl Backend for Fallible {
    async fn get(&self, k: &str) -> Result<Option<Vec<u8>>, StoreError> {
        self.inner.get(k).await
    }
    async fn put(&self, k: &str, v: &[u8]) -> Result<(), StoreError> {
        self.inner.put(k, v).await
    }
    async fn delete(&self, k: &str) -> Result<(), StoreError> {
        self.inner.delete(k).await
    }
    async fn list(&self, k: &str) -> Result<Vec<String>, StoreError> {
        self.inner.list(k).await
    }
    async fn scan(&self, a: &str, b: &str) -> Result<Vec<String>, StoreError> {
        self.inner.scan(a, b).await
    }
    async fn apply(&self, v: &[WriteOp]) -> Result<(), StoreError> {
        self.inner.apply(v).await
    }
    async fn transact(&self, f: TransactFn) -> Result<(), StoreError> {
        if self.nontransactional {
            return Err(StoreError::NotTransactional);
        }
        if self.fail.swap(false, Ordering::AcqRel) {
            return Err(StoreError::Backend("planted commit failure".into()));
        }
        self.inner.transact(f).await
    }
}

#[test]
fn failed_intent_or_proposal_commit_never_exposes_dispatch_or_review() {
    pollster::block_on(async {
        let backend = Fallible::default();
        let owner = owner("fixture");
        let inputs = passages(&owner);
        let auth = authority(&owner, ID, &inputs);
        let shop = Workshop::new(backend.clone(), owner, ID);
        shop.install(SUBJECT).await.unwrap();
        shop.begin(1, inputs, &auth, 0).await.unwrap();
        backend.fail.store(true, Ordering::Release);
        assert!(matches!(
            shop.dispatch(1, &auth, 1).await,
            Err(Error::Store(_))
        ));
        assert_eq!(shop.view(Some(1)).await.unwrap().usage.decisions, 0);
        for n in 0..2 {
            let result = evaluate(shop.dispatch(1, &auth, 2 + n * 2).await.unwrap()).unwrap();
            shop.observe(result, &auth, 3 + n * 2).await.unwrap();
        }
        let result = evaluate(shop.dispatch(1, &auth, 6).await.unwrap()).unwrap();
        backend.fail.store(true, Ordering::Release);
        assert!(matches!(
            shop.observe(result.clone(), &auth, 7).await,
            Err(Error::Store(_))
        ));
        let view = shop.view(Some(1)).await.unwrap();
        assert_eq!(view.review, Some(Review::Generating));
        assert!(view.proposal.is_none());
        shop.observe(result, &auth, 8).await.unwrap();
        assert_eq!(
            shop.view(Some(1)).await.unwrap().review,
            Some(Review::AwaitingReview)
        );
    });
}

#[test]
fn a_nontransactional_backend_refuses_installation() {
    pollster::block_on(async {
        let backend = Fallible {
            nontransactional: true,
            ..Default::default()
        };
        let shop = Workshop::new(backend.clone(), owner("fixture"), ID);
        assert!(matches!(shop.install(SUBJECT).await, Err(Error::Store(_))));
        assert!(backend.inner.is_empty());
    });
}

#[test]
fn two_owner_installations_share_definition_and_never_share_authority() {
    pollster::block_on(async {
        let store = MemoryBackend::new();
        let a = owner("host-a");
        let b = owner("host-b");
        let ia = passages(&a);
        let ib = passages(&b);
        let aa = authority(&a, ID, &ia);
        let ab = authority(&b, [4; 16], &ib);
        let sa = Workshop::new(store.clone(), a, ID);
        let sb = Workshop::new(store, b, [4; 16]);
        sa.install(SUBJECT).await.unwrap();
        sb.install(SUBJECT).await.unwrap();
        assert_eq!(
            sb.begin(1, ib.clone(), &aa, 0).await,
            Err(Error::Unauthorized)
        );
        assert!(sb.begin(1, ia.clone(), &aa, 0).await.is_err());
        sa.begin(1, ia, &aa, 0).await.unwrap();
        sb.begin(1, ib, &ab, 0).await.unwrap();
        generate(&sa, &aa, 1).await;
        generate(&sb, &ab, 1).await;
        let va = sa.view(Some(1)).await.unwrap();
        let vb = sb.view(Some(1)).await.unwrap();
        assert_eq!(
            va.proposal.unwrap().definition,
            vb.proposal.unwrap().definition
        );
        assert_ne!(va.installation, vb.installation);
    });
}

#[test]
fn cold_process_child() {
    let Ok(path) = std::env::var("ALEMBIC_WORKSHOP_TEST_STORE") else {
        return;
    };
    pollster::block_on(async {
        let backend = muniment::DirectoryBackend::open(path).unwrap();
        let owner = owner("cold-process-fixture");
        let inputs = passages(&owner);
        let auth = authority(&owner, ID, &inputs);
        let shop = Workshop::new(backend, owner, ID);
        if std::env::var("ALEMBIC_WORKSHOP_TEST_PHASE").unwrap() == "seed" {
            shop.install(SUBJECT).await.unwrap();
            shop.begin(1, inputs, &auth, 0).await.unwrap();
            generate(&shop, &auth, 1).await;
            shop.accept(1, destination(&shop.owner), HUMAN, &auth, 8)
                .await
                .unwrap();
            shop.uncertain(1).await.unwrap();
        } else {
            let view = shop.view(Some(1)).await.unwrap();
            assert_eq!(view.phase, RunStatus::Completed);
            assert!(matches!(view.review, Some(Review::Reconciling(_))));
            assert!(
                view.proposal
                    .unwrap()
                    .djot
                    .contains("The café note cites version 2 of its source.")
            );
            let assigned = shop
                .accept(1, destination(&shop.owner), HUMAN, &auth, 8)
                .await
                .unwrap();
            assert_eq!(
                assigned,
                shop.accept(1, destination(&shop.owner), HUMAN, &auth, 8)
                    .await
                    .unwrap()
            );
            assert_eq!(view.installation, ID);
            assert_eq!(view.generation, 1);
        }
    });
}

#[test]
fn independent_processes_reopen_exact_proposal_and_uncertain_acceptance() {
    let directory = tempfile::tempdir().unwrap();
    for phase in ["seed", "reopen"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "workshop::tests::cold_process_child",
                "--nocapture",
            ])
            .env("ALEMBIC_WORKSHOP_TEST_STORE", directory.path())
            .env("ALEMBIC_WORKSHOP_TEST_PHASE", phase)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[cfg(feature = "workshop-worker")]
#[test]
fn armillary_driver_uses_the_same_body_and_observes_join() {
    pollster::block_on(async {
        let (shop, _, auth, inputs) = fixture();
        shop.install(SUBJECT).await.unwrap();
        shop.begin(1, inputs, &auth, 0).await.unwrap();
        let worker = worker::Worker::spawn(Arc::new(|| {}));
        for n in 0..3 {
            assert!(worker.dispatch(shop.dispatch(1, &auth, 1 + n * 2).await.unwrap()));
            let observed = worker
                .observations
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap()
                .unwrap();
            shop.observe(observed, &auth, 2 + n * 2).await.unwrap();
        }
        worker.cancel();
        worker.join().unwrap();
        assert_eq!(
            shop.view(Some(1)).await.unwrap().review,
            Some(Review::AwaitingReview)
        );
    });
}

#[test]
fn a_driver_chooses_and_orders_passages_and_the_owner_accepts_only_that_choice() {
    pollster::block_on(async {
        let (shop, backend, auth, inputs) = fixture();
        shop.install(SUBJECT).await.unwrap();
        shop.begin_model(1, inputs.clone(), &auth, 10, 64)
            .await
            .unwrap();
        let first = shop.dispatch(1, &auth, 11).await.unwrap();
        shop.observe(evaluate(first).unwrap(), &auth, 12)
            .await
            .unwrap();

        let second = shop.dispatch(1, &auth, 13).await.unwrap();
        assert_eq!(
            operations(&second)["properties"]["order"]["items"]["maximum"],
            2
        );
        assert!(context(&second).starts_with("Passage 0: Field notes\nA retained copy"));
        let driver =
            crate::decision::Scripted::new([serde_json::json!({"op": "brief", "order": [2, 0]})]);
        let proposed = propose_with(&driver, second, 64).unwrap();
        assert_eq!(proposed.decision.provider, "athanor.scripted");
        let chosen = vec![inputs[2].clone(), inputs[0].clone()];
        assert_eq!(
            proposed.observation.proposal.as_ref().unwrap().sources,
            chosen
        );

        // A passage the owner never selected cannot enter through a worker.
        let mut stray = inputs[1].clone();
        stray.source.resource = "outside-grant".into();
        let mut outside = proposed.observation.clone();
        outside.proposal = Some(proposal(ID, 1, &[inputs[0].clone(), stray]).unwrap());
        assert_eq!(
            shop.observe(outside, &auth, 14).await,
            Err(Error::Invalid("worker proposal"))
        );
        shop.observe(proposed.observation, &auth, 14).await.unwrap();

        // Step 2 confirms the recorded candidate and nothing else.
        let third = shop.dispatch(1, &auth, 15).await.unwrap();
        let confirmed = evaluate(third).unwrap();
        let mut swapped = confirmed.clone();
        swapped.proposal = Some(proposal(ID, 1, &inputs).unwrap());
        assert_eq!(
            shop.observe(swapped, &auth, 16).await,
            Err(Error::Invalid("worker proposal"))
        );
        shop.observe(confirmed, &auth, 16).await.unwrap();

        let reopened = Workshop::new(backend, shop.owner.clone(), ID);
        let view = reopened.view(Some(1)).await.unwrap();
        assert_eq!(view.review, Some(Review::AwaitingReview));
        let brief = view.proposal.unwrap();
        assert_eq!(brief.sources, chosen);
        validate_proposal(&brief).unwrap();
        assert!(
            brief.djot.find("Acceptance finishes").unwrap()
                < brief.djot.find("A retained copy").unwrap()
        );
        assert!(!brief.djot.contains("The café note"));
    });
}

#[test]
fn decisions_outside_the_offered_choice_and_unfit_dispatches_are_refused() {
    use crate::decision::{DecisionError, Scripted};
    use serde_json::json;
    pollster::block_on(async {
        let (shop, _, auth, inputs) = fixture();
        shop.install(SUBJECT).await.unwrap();
        shop.begin(1, inputs, &auth, 0).await.unwrap();
        let script = |value| Scripted::new([value]);
        let fine = || json!({"op": "brief", "order": [0, 1]});

        let zero = shop.dispatch(1, &auth, 1).await.unwrap();
        assert_eq!(
            propose_with(&script(fine()), zero.clone(), 8).unwrap_err(),
            ProposeError::Workshop(Error::Invalid("workflow step"))
        );
        shop.observe(evaluate(zero).unwrap(), &auth, 2)
            .await
            .unwrap();

        let one = shop.dispatch(1, &auth, 3).await.unwrap();
        for bad in [
            json!({"op": "brief", "order": [0, 0]}),
            json!({"op": "brief", "order": [1]}),
            json!({"op": "brief", "order": [0, 3]}),
            json!({"op": "brief", "order": [0, -1]}),
            json!({"op": "drop", "order": [0, 1]}),
            json!({"op": "brief", "order": [0, 1], "extra": 1}),
            json!("brief"),
        ] {
            assert!(
                matches!(
                    propose_with(&script(bad.clone()), one.clone(), 8),
                    Err(ProposeError::Driver(DecisionError::Invalid(_)))
                ),
                "{bad}"
            );
        }
        assert!(matches!(
            propose_with(&Scripted::default(), one.clone(), 8),
            Err(ProposeError::Driver(DecisionError::Failed(_)))
        ));
        let mut private = one;
        private.protected = true;
        assert_eq!(
            propose_with(&script(fine()), private, 8).unwrap_err(),
            ProposeError::Workshop(Error::Invalid("private storage is not qualified"))
        );
    });
}

/// A real model at the proposing step (decision-driver P1, D10). Ignored: it
/// needs the pinned Qwen3-0.6B checkpoint and a discrete GPU.
///
/// `ESP_QWEN3_DIR=/path/to/Qwen3-0.6B cargo test --features workshop-model-gpu --lib a_model_proposes -- --ignored --nocapture`
#[cfg(feature = "workshop-model-gpu")]
#[test]
#[ignore = "needs the pinned Qwen3-0.6B checkpoint (ESP_QWEN3_DIR) and a discrete GPU"]
fn a_model_proposes_and_the_workshop_holds_it_to_the_owners_selection() {
    use crate::decision::DecisionError;
    use sha2::{Digest as _, Sha256};
    let dir = std::path::PathBuf::from(std::env::var_os("ESP_QWEN3_DIR").expect("ESP_QWEN3_DIR"));
    let pinned = |name: &str, expected: &str| {
        let bytes = std::fs::read(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        let actual: String = Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, expected, "{name} is not the pinned artifact");
        bytes
    };
    let model = distillery::decision::discrete_gpu_decider(
        &pinned(
            "config.json",
            "660db3b73d788119c04535e48cf9be5f55bc3100841a718637ae695b442f27dd",
        ),
        &pinned(
            "tokenizer.json",
            "aeb13307a71acd8fe81861d94ad54ab689df773318809eed3cbe794b4492dae4",
        ),
        &pinned(
            "model.safetensors",
            "f47f71177f32bcd101b7573ec9171e6a57f4f4d31148d38e382306f42996874b",
        ),
        "Qwen/Qwen3-0.6B",
        Some("<|im_end|>"),
        // The model's own chat template (D8), from its pinned tokenizer config.
        distillery::decision::chat_prompt(
            &pinned(
                "tokenizer_config.json",
                "d5d09f07b48c3086c508b30d1c9114bd1189145b74e982a265350c923acd8101",
            ),
            "You choose which passages an evidence brief cites. Reply with one JSON object.".into(),
            "Which passages are about retained copies and storage? Give their numbers, most \
             relevant first, as {\"op\":\"brief\",\"order\":[...]}."
                .into(),
            serde_json::json!({"enable_thinking": false}),
        )
        .unwrap(),
    )
    .unwrap();

    pollster::block_on(async {
        let (shop, backend, auth, inputs) = fixture();
        shop.install(SUBJECT).await.unwrap();
        // A model run with the cap the model is given: limit 64 (D12, D15).
        shop.begin_model(1, inputs.clone(), &auth, 10, 32)
            .await
            .unwrap();
        let first = shop.dispatch(1, &auth, 11).await.unwrap();
        shop.observe(evaluate(first).unwrap(), &auth, 12)
            .await
            .unwrap();
        let second = shop.dispatch(1, &auth, 13).await.unwrap();
        match propose_with(&model, second, 32) {
            Ok(proposed) => {
                println!(
                    "decision {} from {}, {} tokens",
                    proposed.decision.value, proposed.decision.provider, proposed.decision.tokens
                );
                assert!(proposed.decision.provider.starts_with("Qwen/Qwen3-0.6B"));
                assert!(proposed.decision.tokens > 0);
                let chosen = proposed.observation.proposal.as_ref().unwrap().sources.clone();
                assert!(chosen.len() >= 2 && chosen.iter().all(|p| inputs.contains(p)));
                shop.observe(proposed.observation, &auth, 14).await.unwrap();
                let third = shop.dispatch(1, &auth, 15).await.unwrap();
                shop.observe(evaluate(third).unwrap(), &auth, 16)
                    .await
                    .unwrap();
                let view = Workshop::new(backend, shop.owner.clone(), ID)
                    .view(Some(1))
                    .await
                    .unwrap();
                assert_eq!(view.review, Some(Review::AwaitingReview));
                let turn = &view.turns[0];
                assert!(turn.provider.as_deref().is_some_and(|p| p.starts_with("Qwen/Qwen3-0.6B")));
                assert!(turn.tokens > 0 && turn.tokens <= turn.reserved);
                let brief = view.proposal.unwrap();
                assert_eq!(brief.sources, chosen);
                validate_proposal(&brief).unwrap();
            },
            // A schema-valid decision can still repeat a passage; the adapter
            // refuses it by name and the run is left where it was.
            Err(ProposeError::Driver(DecisionError::Invalid(detail))) => {
                println!("decision refused: {detail}");
                assert_eq!(shop.view(Some(1)).await.unwrap().review, Some(Review::Generating));
            },
            Err(other) => panic!("{other}"),
        }
    });
}
