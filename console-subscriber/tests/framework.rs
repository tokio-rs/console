//! Framework tests
//!
//! The tests in this module are here to verify the testing framework itself.
//! As such, some of these tests may be repeated elsewhere (where we wish to
//! actually test the functionality of `console-subscriber`) and others are
//! negative tests that should panic.

use std::time::Duration;

use tokio::{task, time::sleep};

mod support;
use support::{assert_task, assert_tasks, ExpectedTask, MAIN_TASK_NAME};

#[test]
fn expect_present() {
    let expected_task = ExpectedTask::default()
        .match_name(MAIN_TASK_NAME.into())
        .expect_present();

    let future = async {
        sleep(Duration::ZERO).await;
    };

    assert_task(expected_task, future);
}

#[test]
#[should_panic(expected = "Test failed: Task validation failed:
 - Task<name=main>: no expectations set, if you want to just expect that a matching task is present, use `expect_present()`
")]
fn fail_no_expectations() {
    let expected_task = ExpectedTask::default().match_name(MAIN_TASK_NAME.into());

    let future = async {
        sleep(Duration::ZERO).await;
    };

    assert_task(expected_task, future);
}

#[test]
fn wakes() {
    let expected_task = ExpectedTask::default()
        .match_name(MAIN_TASK_NAME.into())
        .expect_wakes(1);

    let future = async {
        sleep(Duration::ZERO).await;
    };

    assert_task(expected_task, future);
}

#[test]
#[should_panic(expected = "Test failed: Task validation failed:
 - Task<name=main>: expected `wakes` to be 5, but actual was 1
")]
fn fail_wakes() {
    let expected_task = ExpectedTask::default()
        .match_name(MAIN_TASK_NAME.into())
        .expect_wakes(5);

    let future = async {
        sleep(Duration::ZERO).await;
    };

    assert_task(expected_task, future);
}

#[test]
fn self_wakes() {
    let expected_task = ExpectedTask::default()
        .match_name(MAIN_TASK_NAME.into())
        .expect_self_wakes(1);

    let future = async { task::yield_now().await };

    assert_task(expected_task, future);
}

#[test]
#[should_panic(expected = "Test failed: Task validation failed:
 - Task<name=main>: expected `self_wakes` to be 1, but actual was 0
")]
fn fail_self_wake() {
    let expected_task = ExpectedTask::default()
        .match_name(MAIN_TASK_NAME.into())
        .expect_self_wakes(1);

    let future = async {
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
            .spawn(async { task::yield_now().await })
    };

    assert_task(expected_task, future);
}

#[test]
#[should_panic(expected = "Test failed: Task validation failed:
 - Task<name=wrong-name>: no matching actual task was found
")]
fn fail_wrong_task_name() {
    let expected_task = ExpectedTask::default().match_name("wrong-name".into());

    let future = async { task::yield_now().await };

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
            .spawn(async { task::yield_now().await })
            .unwrap();
        let task2 = task::Builder::new()
            .name("task-2")
            .spawn(async { task::yield_now().await })
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
 - Task<name=task-2>: expected `wakes` to be 2, but actual was 1
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
            .spawn(async { task::yield_now().await })
            .unwrap();
        let task2 = task::Builder::new()
            .name("task-2")
            .spawn(async { task::yield_now().await })
            .unwrap();

        tokio::try_join! {
            task1,
            task2,
        }
        .unwrap();
    };

    assert_tasks(expected_tasks, future);
}
