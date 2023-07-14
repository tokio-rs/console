//! Framework tests
//!
//! The tests in this module are here to verify the testing framework itself.
//! As such, some of these tests may be repeated elsewhere (where we wish to
//! actually test the functionality of `console-subscriber`) and others are
//! negative tests that should panic.

use std::time::Duration;

use tokio::{task, time::sleep};

mod support {
    pub mod subscriber;
    pub mod task;
}

use support::subscriber::{assert_tasks, MAIN_TASK_NAME};
use support::task::ExpectedTask;

#[test]
fn wakes() {
    let expected_task = ExpectedTask::default()
        .match_name(MAIN_TASK_NAME.into())
        .expect_wakes(1);
    let expected_tasks = vec![expected_task];

    let future = async {
        sleep(Duration::ZERO).await;
    };

    assert_tasks(expected_tasks, future);
}

#[test]
#[should_panic(expected = "Test failed: Task validation failed:
 - Task<name=main>: expected `wakes` to be 5, but actual was 1
")]
fn fail_wakes() {
    let expected_task = ExpectedTask::default()
        .match_name(MAIN_TASK_NAME.into())
        .expect_wakes(5);
    let expected_tasks = vec![expected_task];

    let future = async {
        sleep(Duration::ZERO).await;
    };

    assert_tasks(expected_tasks, future);
}

#[test]
fn self_wakes() {
    let expected_task = ExpectedTask::default()
        .match_name(MAIN_TASK_NAME.into())
        .expect_self_wakes(1);
    let expected_tasks = vec![expected_task];

    let future = async { task::yield_now().await };

    assert_tasks(expected_tasks, future);
}

#[test]
#[should_panic(expected = "Test failed: Task validation failed:
 - Task<name=main>: expected `self_wakes` to be 1, but actual was 0
")]
fn fail_self_wake() {
    let expected_task = ExpectedTask::default()
        .match_name(MAIN_TASK_NAME.into())
        .expect_self_wakes(1);
    let expected_tasks = vec![expected_task];

    let future = async {
        sleep(Duration::ZERO).await;
    };

    assert_tasks(expected_tasks, future);
}

#[test]
fn test_spawned_task() {
    let expected_task = ExpectedTask::default()
        .match_name("another-name".into())
        .expect_present();
    let expected_tasks = vec![expected_task];

    let future = async {
        task::Builder::new()
            .name("another-name")
            .spawn(async { task::yield_now().await })
    };

    assert_tasks(expected_tasks, future);
}

#[test]
#[should_panic(expected = "Test failed: Task validation failed:
 - Task<name=wrong-name>: no matching actual task was found
")]
fn fail_wrong_task_name() {
    let expected_task = ExpectedTask::default().match_name("wrong-name".into());
    let expected_tasks = vec![expected_task];

    let future = async { task::yield_now().await };

    assert_tasks(expected_tasks, future);
}

#[test]
fn expect_present() {
    let expected_task = ExpectedTask::default()
        .match_name(MAIN_TASK_NAME.into())
        .expect_present();
    let expected_tasks = vec![expected_task];

    let future = async {
        sleep(Duration::ZERO).await;
    };

    assert_tasks(expected_tasks, future);
}

#[test]
#[should_panic(expected = "Test failed: Task validation failed:
 - Task<name=main>: no expectations set, if you want to just expect that a matching task is present, use `expect_present()`
")]
fn fail_no_expectations() {
    let expected_task = ExpectedTask::default().match_name(MAIN_TASK_NAME.into());
    let expected_tasks = vec![expected_task];

    let future = async {
        sleep(Duration::ZERO).await;
    };

    assert_tasks(expected_tasks, future);
}
