// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The two limit profiles (decision-driver D15): a model-free run keeps
//! the pre-P2 limits and is never given a reservation or a driver; a model
//! run reserves exactly its cap at every proposing intent.

use super::*;

fn failed_turn(correlation: Correlation) -> Observation {
    Observation {
        correlation,
        proposal: None,
        turn: Some(turn::Outcome::Failed("no answer".into())),
    }
}

#[test]
fn the_proposing_step_reserves_its_cap_at_intent_and_servitor_refuses_an_unaffordable_one() {
    pollster::block_on(async {
        let store = MemoryBackend::new();
        let shop = Workshop::new(store, owner(), ID);
        shop.install(SUBJECT).await.unwrap();
        shop.begin_model(1, inputs(), &Granted, 0, 32)
            .await
            .unwrap();
        let view = shop.view(Some(1)).await.unwrap();
        assert_eq!(view.limits, model_limits(32));
        assert_eq!(view.limits.unwrap().tokens, 64);
        let zero = shop.dispatch(1, &Granted, 1).await.unwrap();
        shop.observe(evaluate(zero).unwrap(), &Granted, 2)
            .await
            .unwrap();
        // Every proposing attempt reserves the cap at intent, an interrupted
        // one included: the interrupted attempt and its retry take all 64.
        shop.dispatch(1, &Granted, 3).await.unwrap();
        assert_eq!(shop.view(Some(1)).await.unwrap().usage.tokens, 32);
        shop.interrupt(1, 4).await.unwrap();
        let retry = shop.dispatch(1, &Granted, 5).await.unwrap();
        assert_eq!(shop.view(Some(1)).await.unwrap().usage.tokens, 64);
        shop.observe(failed_turn(retry.correlation), &Granted, 6)
            .await
            .unwrap();
        // The failure leaves a retry and a decision, but no tokens: Servitor
        // refuses the third reservation and the run's budget is exhausted.
        let view = shop.view(Some(1)).await.unwrap();
        assert_eq!(view.phase, RunStatus::Ready(proposing(2)));
        assert_eq!((view.usage.decisions, view.usage.tokens), (3, 64));
        assert_eq!(
            shop.dispatch(1, &Granted, 7).await.unwrap_err(),
            Error::Budget
        );
        let view = shop.view(Some(1)).await.unwrap();
        assert_eq!(view.phase, RunStatus::BudgetExhausted);
        assert_eq!(view.usage.tokens, 64);
    });
}

#[test]
fn a_model_free_run_through_the_driver_behaves_as_before() {
    pollster::block_on(async {
        let (send, wake) = mpsc::channel();
        let notify = Arc::new(move || {
            let _ = send.send(());
        });
        let mut host = Driver::new(MemoryBackend::new(), owner(), ID, notify);
        let time = AtomicU64::new(1);
        let clock = ticking(&time);
        host.open(SUBJECT).await.unwrap();
        host.begin(inputs(), &Granted, &clock, &|_| true)
            .await
            .unwrap();
        let view = settle(&mut host, &wake, &clock).await.unwrap();
        assert_eq!(view.phase, RunStatus::Completed);
        assert_eq!(view.limits, Some(LIMITS));
        assert_eq!(
            (
                view.usage.decisions,
                view.usage.tool_calls,
                view.usage.tokens,
                view.usage.consecutive_failures
            ),
            (3, 2, 0, 0)
        );
        assert!(view.turns.is_empty());
        assert_eq!(view.proposal.unwrap().sources, inputs());
    });
}

#[test]
fn a_run_begun_without_a_driver_is_never_given_reservations_or_a_driver() {
    pollster::block_on(async {
        let store = MemoryBackend::new();
        let shop = Workshop::new(store.clone(), owner(), ID);
        shop.install(SUBJECT).await.unwrap();
        shop.begin(1, inputs(), &Granted, 0).await.unwrap();
        let zero = shop.dispatch(1, &Granted, 1).await.unwrap();
        shop.observe(evaluate(zero).unwrap(), &Granted, 2)
            .await
            .unwrap();
        let one = shop.dispatch(1, &Granted, 3).await.unwrap();
        assert_eq!(shop.view(Some(1)).await.unwrap().usage.tokens, 0);

        // A driver's result or failure is refused on a model-free run.
        let driven = propose_with(&Scripted::new([brief(&[0, 1])]), one.clone(), 32).unwrap();
        assert_eq!(
            shop.observe(driven.observation, &Granted, 4).await,
            Err(Error::Invalid("model usage"))
        );
        assert_eq!(
            shop.observe(failed_turn(one.correlation), &Granted, 4)
                .await,
            Err(Error::Invalid("failed turn"))
        );

        // A host with a decider will not resume it, and never asks its driver.
        shop.lifecycle(Control::Pause, &Granted, 5).await.unwrap();
        let counted = Counted::new([Ok(brief(&[0, 1]))]);
        let (mut host, _wake) = rig(&store, counted.clone(), 32);
        let time = AtomicU64::new(6);
        let clock = ticking(&time);
        assert_eq!(
            host.control(Control::Resume, &Granted, &clock, &|_| true)
                .await
                .map(|_| ()),
            Err(Error::Invalid("decision driver profile"))
        );
        assert!(matches!(
            host.refresh().await.unwrap().phase,
            RunStatus::Paused(_)
        ));
        assert_eq!(counted.calls(), 0);

        // A reservation written into its journal is refused at load.
        tamper(&shop, &store, |run| {
            for entry in &mut run.entries {
                if let Entry::Intent {
                    correlation,
                    reserved,
                    ..
                } = entry
                    && correlation.step == 1
                {
                    *reserved = 32;
                }
            }
        })
        .await;
        assert_eq!(
            shop.view(Some(1)).await,
            Err(Error::Invalid("token reservation"))
        );
    });
}

#[test]
fn load_accepts_only_the_two_profiles_and_reservations_that_agree() {
    pollster::block_on(async {
        let store = MemoryBackend::new();
        let shop = Workshop::new(store.clone(), owner(), ID);
        shop.install(SUBJECT).await.unwrap();
        shop.begin_model(1, inputs(), &Granted, 0, 32)
            .await
            .unwrap();
        let zero = shop.dispatch(1, &Granted, 1).await.unwrap();
        shop.observe(evaluate(zero).unwrap(), &Granted, 2)
            .await
            .unwrap();
        shop.dispatch(1, &Granted, 3).await.unwrap();
        let (_, good) = shop.load().await.unwrap();
        let put = |run: Run| {
            let mut record = good.clone();
            record.runs[0] = run;
            let store = store.clone();
            let key = shop.key.clone();
            async move { store.put(&key, &encoded(&record).unwrap()).await.unwrap() }
        };
        let base = good.runs[0].clone();

        // A cap the reservations disagree with: limit 128 says cap 64.
        let mut other = base.clone();
        other.limits = model_limits(64).unwrap();
        put(other).await;
        assert_eq!(
            shop.view(Some(1)).await,
            Err(Error::Invalid("token reservation"))
        );
        // Neither profile: an odd token limit, or a model run's other limits.
        for limits in [
            Limits {
                tokens: 65,
                ..model_limits(32).unwrap()
            },
            Limits {
                decisions: 5,
                ..model_limits(32).unwrap()
            },
            Limits {
                tokens: 64,
                ..LIMITS
            },
        ] {
            let mut other = base.clone();
            other.limits = limits;
            put(other).await;
            assert_eq!(shop.view(Some(1)).await, Err(Error::Invalid("run limits")));
        }
        put(base).await;
        assert_eq!(shop.view(Some(1)).await.unwrap().usage.tokens, 32);
    });
}
