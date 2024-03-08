mod support;
use std::time::Duration;

use support::{assert_task, ExpectedTask};
use tokio::time::sleep;

#[test]
fn sleep_wakes() {
    let expected_task = ExpectedTask::default()
        .match_default_name()
        .expect_wakes(1)
        .expect_self_wakes(0);

    let future = async {
        sleep(Duration::ZERO).await;
    };

    assert_task(expected_task, future);
}

#[test]
fn double_sleep_wakes() {
    let expected_task = ExpectedTask::default()
        .match_default_name()
        .expect_wakes(2)
        .expect_self_wakes(0);

    let future = async {
        sleep(Duration::ZERO).await;
        sleep(Duration::ZERO).await;
    };

    assert_task(expected_task, future);
}

#[test]
fn self_wake() {
    let expected_task = ExpectedTask::default()
        .match_default_name()
        .expect_wakes(1)
        .expect_self_wakes(1);

    let future = async {
        support::self_wake().await;
    };

    assert_task(expected_task, future);
}
