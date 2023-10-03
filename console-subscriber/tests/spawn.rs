// use std::time::Duration;

// use tokio::time::sleep;

// mod support;
// use support::{assert_tasks, spawn_named, ExpectedTask};

// /// This test asserts the behavior that was fixed in #440. Before that fix,
// /// the polls of a child were also counted towards the parent (the task which
// /// spawned the child task). In this scenario, that would result in the parent
// /// having 3 polls counted, when it should really be 1.
// #[test]
// fn child_polls_dont_count_towards_parent_polls() {
//     let expected_tasks = vec![
//         ExpectedTask::default()
//             .match_name("parent".into())
//             .expect_polls(1),
//         ExpectedTask::default()
//             .match_name("child".into())
//             .expect_polls(2),
//     ];

//     let future = async {
//         let child_join_handle = spawn_named("parent", async {
//             spawn_named("child", async {
//                 sleep(Duration::ZERO).await;
//             })
//         })
//         .await
//         .expect("joining parent failed");

//         child_join_handle.await.expect("joining child failed");
//     };

//     assert_tasks(expected_tasks, future);
// }
