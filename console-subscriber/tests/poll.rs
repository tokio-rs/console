use std::time::Duration;

use tokio::time::sleep;

mod support;
use support::{assert_task, ExpectedTask};

#[test]
fn single_poll() {
    // There is an extra poll because the span enters one more time upon drop (see
    // tokio-rs/tracing#2562).
    let expected_task = ExpectedTask::default().match_default_name().expect_polls(2);

    let future = futures::future::ready(());

    assert_task(expected_task, future);
}

#[test]
fn two_polls() {
    // There is an extra poll because the span enters one more time upon drop (see
    // tokio-rs/tracing#2562).
    let expected_task = ExpectedTask::default().match_default_name().expect_polls(3);

    let future = async {
        sleep(Duration::ZERO).await;
    };

    assert_task(expected_task, future);
}

#[test]
fn many_polls() {
    // There is an extra poll because the span enters one more time upon drop (see
    // tokio-rs/tracing#2562).
    let expected_task = ExpectedTask::default()
        .match_default_name()
        .expect_polls(12);

    let future = async {
        for _ in 0..10 {
            sleep(Duration::ZERO).await;
        }
    };

    assert_task(expected_task, future);
}
