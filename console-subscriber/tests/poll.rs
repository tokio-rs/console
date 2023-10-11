use std::time::Duration;

use tokio::time::sleep;

mod support;
use support::{assert_task, ExpectedTask};

#[test]
fn single_poll() {
    let expected_task = ExpectedTask::default().match_default_name().expect_polls(1);

    let future = futures::future::ready(());

    assert_task(expected_task, future);
}

#[test]
fn two_polls() {
    let expected_task = ExpectedTask::default().match_default_name().expect_polls(2);

    let future = async {
        sleep(Duration::ZERO).await;
    };

    assert_task(expected_task, future);
}

#[test]
fn many_polls() {
    let expected_task = ExpectedTask::default()
        .match_default_name()
        .expect_polls(11);

    let future = async {
        for _ in 0..10 {
            sleep(Duration::ZERO).await;
        }
    };

    assert_task(expected_task, future);
}
