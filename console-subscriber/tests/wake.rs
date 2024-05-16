mod support;

use support::{assert_task, ExpectedTask};

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
