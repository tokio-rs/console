use futures::Future;

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
