use std::{future::Future, task::Poll};

use tokio::task::JoinHandle;

mod state;
mod subscriber;
mod task;

use subscriber::run_test;
pub(crate) use subscriber::MAIN_TASK_NAME;
pub(crate) use task::ExpectedTask;

/// Assert that an `expected_task` is recorded by a console-subscriber
/// when driving the provided `future` to completion.
///
/// This function is equivalent to calling [`assert_tasks`] with a vector
/// containing a single task.
///
/// # Panics
///
/// This function will panic if the expectations on the expected task are not
/// met or if a matching task is not recorded.
#[track_caller]
#[allow(dead_code)]
pub(crate) fn assert_task<Fut>(expected_task: ExpectedTask, future: Fut)
where
    Fut: Future + Send + 'static,
    Fut::Output: Send + 'static,
{
    run_test(vec![expected_task], future)
}

/// Assert that the `expected_tasks` are recorded by a console-subscriber
/// when driving the provided `future` to completion.
///
/// # Panics
///
/// This function will panic if the expectations on any of the expected tasks
/// are not met or if matching tasks are not recorded for all expected tasks.
#[track_caller]
#[allow(dead_code)]
pub(crate) fn assert_tasks<Fut>(expected_tasks: Vec<ExpectedTask>, future: Fut)
where
    Fut: Future + Send + 'static,
    Fut::Output: Send + 'static,
{
    run_test(expected_tasks, future)
}

/// Spawn a named task and unwrap.
///
/// This is a convenience function to create a task with a name and then spawn
/// it directly (unwrapping the `Result` which the task builder API returns).
#[allow(dead_code)]
pub(crate) fn spawn_named<Fut>(name: &str, f: Fut) -> JoinHandle<<Fut as Future>::Output>
where
    Fut: Future + Send + 'static,
    Fut::Output: Send + 'static,
{
    tokio::task::Builder::new()
        .name(name)
        .spawn(f)
        .expect(&format!("spawning task '{name}' failed"))
}

/// Wakes itself from within this task.
///
/// This function returns a future which will wake itself and then
/// return `Poll::Pending` the first time it is called. The next time
/// it will return `Poll::Ready`.
///
/// This is the old behavior of Tokio's [`yield_now()`] function, before it
/// was improved in [tokio-rs/tokio#5223] to avoid starving the resource
/// drivers.
///
/// Awaiting the future returned from this function will result in a
/// self-wake being recorded.
///
/// [`yield_now()`]: fn@tokio::task::yield_now
/// [tokio-rs/tokio#5223]: https://github.com/tokio-rs/tokio/pull/5223
#[allow(dead_code)]
pub(crate) fn self_wake() -> impl Future<Output = ()> {
    struct SelfWake {
        yielded: bool,
    }

    impl Future for SelfWake {
        type Output = ();

        fn poll(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> Poll<Self::Output> {
            if self.yielded == true {
                return Poll::Ready(());
            }

            self.yielded = true;
            cx.waker().wake_by_ref();

            Poll::Pending
        }
    }

    SelfWake { yielded: false }
}
