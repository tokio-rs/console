use std::time::Duration;

use tokio::{task, time::sleep};

mod support {
    pub mod subscriber;
    pub mod task;
}

use support::subscriber::{assert_tasks, MAIN_TASK_NAME};
use support::task::ExpectedTask;

#[test]
fn self_wake() {
    // Test is here
    let expected_task = ExpectedTask::default()
        .match_name(MAIN_TASK_NAME.into())
        .expect_wakes(1)
        .expect_self_wakes(1);
    let expected_tasks = vec![expected_task];

    let future = async { task::yield_now().await };

    assert_tasks(expected_tasks, future);
}

#[test]
fn test_spawned_task() {
    // Test is here
    let expected_task = ExpectedTask::default()
        .match_name("another-name".into())
        .expect_wakes(1)
        .expect_self_wakes(1);
    let expected_tasks = vec![expected_task];

    let future = async {
        task::Builder::new()
            .name("another-name")
            .spawn(async { task::yield_now().await })
    };

    assert_tasks(expected_tasks, future);
}

#[test]
#[should_panic(expected = "Test failed: No tasks matched the expected tasks.")]
fn fail_wrong_task_name() {
    // Test is here
    let expected_task = ExpectedTask::default()
        .match_name("wrong-name".into())
        .expect_wakes(1)
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
    // Test is here
    let expected_task = ExpectedTask::default()
        .match_name(MAIN_TASK_NAME.into())
        .expect_wakes(1)
        .expect_self_wakes(1);
    let expected_tasks = vec![expected_task];

    let future = async {
        // The test starts here.
        sleep(Duration::ZERO).await;
        // The test ends here.
    };

    assert_tasks(expected_tasks, future);
}
