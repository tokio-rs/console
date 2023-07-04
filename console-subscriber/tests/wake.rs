use std::time::Duration;

use tokio::{task, time::sleep};

mod support {
    pub mod subscriber;
    pub mod subscriber2;
    pub mod task;
}

use support::subscriber::TestSubscriber;
use support::subscriber2::assert_tasks;
use support::task::ExpectedTask;

// #[test]
// fn broken_self_wake() {
//     let expected_tasks = vec![ExpectedTask::default()
//         .match_name("main".into())
//         .expect_wakes(1)
//         .expect_self_wakes(1)];

//     TestSubscriber::new().assert(expected_tasks, async {
//         // The test starts here.
//         task::yield_now().await
//         // The test ends here.
//     });
// }

#[test]
fn self_wake() {
    // Test is here
    let expected_task = ExpectedTask::default()
        .match_name("mog".into())
        .expect_wakes(1)
        .expect_self_wakes(1);
    let expected_tasks = vec![expected_task];

    let future = async {
        // The test starts here.
        task::yield_now().await
        // The test ends here.
    };

    assert_tasks(expected_tasks, future);
}

#[test]
fn fail_self_wake() {
    // Test is here
    let expected_task = ExpectedTask::default()
        .match_name("mog".into())
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
