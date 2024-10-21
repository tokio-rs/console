//! Framework tests
//!
//! The tests in this module are here to verify the testing framework itself.
//! As such, some of these tests may be repeated elsewhere (where we wish to
//! actually test the functionality of `console-subscriber`) and others are
//! negative tests that should panic.

use std::time::Duration;

use futures::future;
use tokio::{task, time::sleep};

mod support;
use support::{assert_task, assert_tasks, ExpectedTask};

#[test]
fn expect_present() {
    let expected_task = ExpectedTask::default()
        .match_default_name()
        .expect_present();

    let future = future::ready(());

    assert_task(expected_task, future);
}

#[test]
#[should_panic(expected = "Test failed: Task validation failed:
 - Task { name=console-test::main }: no expectations set, if you want to just expect that a matching task is present, use `expect_present()`
")]
fn fail_no_expectations() {
    let expected_task = ExpectedTask::default().match_default_name();

    let future = future::ready(());

    assert_task(expected_task, future);
}

#[test]
fn wakes() {
    let expected_task = ExpectedTask::default().match_default_name().expect_wakes(1);

    let future = async { yield_to_runtime().await };

    assert_task(expected_task, future);
}

#[test]
#[should_panic(expected = "Test failed: Task validation failed:
 - Task { name=console-test::main }: expected `wakes` to be 5, but actual was 1
")]
fn fail_wakes() {
    let expected_task = ExpectedTask::default().match_default_name().expect_wakes(5);

    let future = async { yield_to_runtime().await };

    assert_task(expected_task, future);
}

#[test]
fn self_wakes() {
    let expected_task = ExpectedTask::default()
        .match_default_name()
        .expect_self_wakes(1);

    let future = async { support::self_wake().await };

    assert_task(expected_task, future);
}

#[test]
#[should_panic(expected = "Test failed: Task validation failed:
 - Task { name=console-test::main }: expected `self_wakes` to be 1, but actual was 0
")]
fn fail_self_wake() {
    let expected_task = ExpectedTask::default()
        .match_default_name()
        .expect_self_wakes(1);

    let future = async {
        // `sleep` doesn't result in a self wake
        sleep(Duration::ZERO).await;
    };

    assert_task(expected_task, future);
}

#[test]
fn test_spawned_task() {
    let expected_task = ExpectedTask::default()
        .match_name("another-name".into())
        .expect_present();

    let future = async {
        task::Builder::new()
            .name("another-name")
            .spawn(async { yield_to_runtime().await })
    };

    assert_task(expected_task, future);
}

#[test]
#[should_panic(expected = "Test failed: Task validation failed:
 - Task { name=wrong-name }: no matching actual task was found
")]
fn fail_wrong_task_name() {
    let expected_task = ExpectedTask::default().match_name("wrong-name".into());

    let future = async { yield_to_runtime().await };

    assert_task(expected_task, future);
}

#[test]
fn multiple_tasks() {
    let expected_tasks = vec![
        ExpectedTask::default()
            .match_name("task-1".into())
            .expect_wakes(1),
        ExpectedTask::default()
            .match_name("task-2".into())
            .expect_wakes(1),
    ];

    let future = async {
        let task1 = task::Builder::new()
            .name("task-1")
            .spawn(async { yield_to_runtime().await })
            .unwrap();
        let task2 = task::Builder::new()
            .name("task-2")
            .spawn(async { yield_to_runtime().await })
            .unwrap();

        tokio::try_join! {
            task1,
            task2,
        }
        .unwrap();
    };

    assert_tasks(expected_tasks, future);
}

#[test]
#[should_panic(expected = "Test failed: Task validation failed:
 - Task { name=task-2 }: expected `wakes` to be 2, but actual was 1
")]
fn fail_1_of_2_expected_tasks() {
    let expected_tasks = vec![
        ExpectedTask::default()
            .match_name("task-1".into())
            .expect_wakes(1),
        ExpectedTask::default()
            .match_name("task-2".into())
            .expect_wakes(2),
    ];

    let future = async {
        let task1 = task::Builder::new()
            .name("task-1")
            .spawn(async { yield_to_runtime().await })
            .unwrap();
        let task2 = task::Builder::new()
            .name("task-2")
            .spawn(async { yield_to_runtime().await })
            .unwrap();

        tokio::try_join! {
            task1,
            task2,
        }
        .unwrap();
    };

    assert_tasks(expected_tasks, future);
}

#[test]
fn polls() {
    // There is an extra poll because the span enters one more time upon drop (see
    // tokio-rs/tracing#2562).
    let expected_task = ExpectedTask::default().match_default_name().expect_polls(3);

    let future = async { yield_to_runtime().await };

    assert_task(expected_task, future);
}

#[test]
#[should_panic(expected = "Test failed: Task validation failed:
 - Task { name=console-test::main }: expected `polls` to be 3, but actual was 2
")]
fn fail_polls() {
    // There is an extra poll because the span enters one more time upon drop (see
    // tokio-rs/tracing#2562).
    let expected_task = ExpectedTask::default().match_default_name().expect_polls(3);

    let future = async {};

    assert_task(expected_task, future);
}

async fn yield_to_runtime() {
    // There is a race condition that can occur when tests are run in parallel,
    // caused by tokio-rs/tracing#2743. It tends to cause test failures only
    // when the test relies on a wake coming from `tokio::task::yield_now()`.
    // For this reason, we prefer a zero-duration sleep.
    sleep(Duration::ZERO).await;
}
