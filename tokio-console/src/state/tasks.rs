use crate::{
    intern::{self, InternedStr},
    state::{
        format_location,
        histogram::DurationHistogram,
        pb_duration,
        store::{self, Id, SpanId, Store},
        Field, FieldValue, Metadata, Visibility,
    },
    util::Percentage,
    view,
    warnings::Linter,
};
use console_api as proto;
use ratatui::{style::Color, text::Span};
use std::{
    cell::RefCell,
    collections::HashMap,
    convert::{TryFrom, TryInto},
    rc::{Rc, Weak},
    time::{Duration, SystemTime},
};

#[derive(Default, Debug)]
pub(crate) struct TasksState {
    tasks: Store<Task>,
    pub(crate) linters: Vec<Linter<Task>>,
    dropped_events: u64,
}

#[derive(Debug, Default)]
pub(crate) struct Details {
    pub(crate) span_id: SpanId,
    pub(crate) poll_times_histogram: Option<DurationHistogram>,
    pub(crate) scheduled_times_histogram: Option<DurationHistogram>,
}

#[derive(Debug, Copy, Clone)]
#[repr(usize)]
pub(crate) enum SortBy {
    Warns = 0,
    Tid = 1,
    State = 2,
    Name = 3,
    Total = 4,
    Busy = 5,
    Scheduled = 6,
    Idle = 7,
    Polls = 8,
    Target = 9,
    Location = 10,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum TaskState {
    Completed,
    Idle,
    Running,
    Scheduled,
}

pub(crate) type TaskRef = store::Ref<Task>;

/// The Id for a Tokio task.
///
/// This should be equivalent to [`tokio::task::Id`], which can't be
/// used because it's not possible to construct outside the `tokio`
/// crate.
///
/// Within the context of `tokio-console`, we don't depend on it
/// being the same as Tokio's own type, as the task id is recorded
/// as a `u64` in tracing and then sent via the wire protocol as such.
pub(crate) type TaskId = u64;

#[derive(Debug)]
pub(crate) struct Task {
    /// The task's pretty (console-generated, sequential) task ID.
    ///
    /// This is NOT the `tracing::span::Id` for the task's tracing span on the
    /// remote.
    id: Id<Task>,
    /// The `tokio::task::Id` in the remote tokio runtime.
    task_id: Option<TaskId>,
    /// The `tracing::span::Id` on the remote process for this task's span.
    ///
    /// This is used when requesting a task details stream.
    span_id: SpanId,
    /// A cached string representation of the Id for display purposes.
    id_str: String,
    short_desc: InternedStr,
    formatted_fields: Vec<Vec<Span<'static>>>,
    stats: TaskStats,
    target: InternedStr,
    name: Option<InternedStr>,
    /// Currently active warnings for this task.
    warnings: Vec<Linter<Task>>,
    location: String,
}

#[derive(Debug)]
struct TaskStats {
    polls: u64,
    created_at: SystemTime,
    dropped_at: Option<SystemTime>,
    busy: Duration,
    scheduled: Duration,
    last_poll_started: Option<SystemTime>,
    last_poll_ended: Option<SystemTime>,
    idle: Option<Duration>,
    total: Option<Duration>,

    // === waker stats ===
    /// Total number of times the task has been woken over its lifetime.
    wakes: u64,
    /// Total number of times the task's waker has been cloned
    waker_clones: u64,

    /// Total number of times the task's waker has been dropped.
    waker_drops: u64,

    /// The timestamp of when the task was last woken.
    last_wake: Option<SystemTime>,
    /// Total number of times the task has woken itself.
    self_wakes: u64,
}

impl TasksState {
    /// Returns any new tasks that were added since the last task update.
    pub(crate) fn take_new_tasks(&mut self) -> impl Iterator<Item = TaskRef> + '_ {
        self.tasks.take_new_items()
    }

    pub(crate) fn ids_mut(&mut self) -> &mut store::Ids<Task> {
        self.tasks.ids_mut()
    }

    pub(crate) fn update_tasks(
        &mut self,
        styles: &view::Styles,
        strings: &mut intern::Strings,
        metas: &HashMap<u64, Metadata>,
        update: proto::tasks::TaskUpdate,
        visibility: Visibility,
    ) {
        let mut stats_update = update.stats_update;
        let linters = &self.linters;

        self.tasks
            .insert_with(visibility, update.new_tasks, |ids, mut task| {
                if task.id.is_none() {
                    tracing::warn!(?task, "skipping task with no id");
                }

                let meta_id = match task.metadata.as_ref() {
                    Some(id) => id.id,
                    None => {
                        tracing::warn!(?task, "task has no metadata ID, skipping");
                        return None;
                    }
                };
                let meta = match metas.get(&meta_id) {
                    Some(meta) => meta,
                    None => {
                        tracing::warn!(?task, meta_id, "no metadata for task, skipping");
                        return None;
                    }
                };
                let mut name = None;
                let mut task_id = None;
                let mut fields = task
                    .fields
                    .drain(..)
                    .filter_map(|pb| {
                        let field = Field::from_proto(pb, meta, strings)?;
                        // the `task.name` field gets its own column, if it's present.
                        if &*field.name == Field::NAME {
                            name = Some(strings.string(field.value.to_string()));
                            return None;
                        }
                        if &*field.name == Field::TASK_ID {
                            task_id = match field.value {
                                FieldValue::U64(id) => Some(id as TaskId),
                                _ => None,
                            };
                            return None;
                        }
                        Some(field)
                    })
                    .collect::<Vec<_>>();

                let formatted_fields = Field::make_formatted(styles, &mut fields);
                let span_id = task.id?.id;

                let stats = stats_update.remove(&span_id)?.into();
                let location = format_location(task.location);

                // remap the server's ID to a pretty, sequential task ID
                let id = ids.id_for(span_id);

                let short_desc = strings.string(match (task_id, name.as_ref()) {
                    (Some(task_id), Some(name)) => format!("{task_id} ({name})"),
                    (Some(task_id), None) => task_id.to_string(),
                    (None, Some(name)) => name.as_ref().to_owned(),
                    (None, None) => "".to_owned(),
                });

                let mut task = Task {
                    name,
                    id,
                    task_id,
                    span_id,
                    id_str: task_id.map(|id| id.to_string()).unwrap_or_default(),
                    short_desc,
                    formatted_fields,
                    stats,
                    target: meta.target.clone(),
                    warnings: Vec::new(),
                    location,
                };
                task.lint(linters);
                Some((id, task))
            });

        for (stats, mut task) in self.tasks.updated(stats_update) {
            tracing::trace!(?task, ?stats, "processing stats update for");
            task.stats = stats.into();
            task.lint(linters);
        }

        self.dropped_events += update.dropped_events;
    }

    pub(crate) fn retain_active(&mut self, now: SystemTime, retain_for: Duration) {
        self.tasks.retain(|_, task| {
            let task = task.borrow();

            task.stats
                .dropped_at
                .map(|d| {
                    let dropped_for = now.duration_since(d).unwrap_or_default();
                    retain_for > dropped_for
                })
                .unwrap_or(true)
        })
    }

    pub(crate) fn warnings(&self) -> impl Iterator<Item = &Linter<Task>> {
        self.linters.iter().filter(|linter| linter.count() > 0)
    }

    pub(crate) fn task(&self, id: Id<Task>) -> Option<TaskRef> {
        self.tasks.get(id).map(Rc::downgrade)
    }

    pub(crate) fn dropped_events(&self) -> u64 {
        self.dropped_events
    }
}

impl Details {
    pub(crate) fn span_id(&self) -> SpanId {
        self.span_id
    }

    pub(crate) fn poll_times_histogram(&self) -> Option<&DurationHistogram> {
        self.poll_times_histogram.as_ref()
    }

    pub(crate) fn scheduled_times_histogram(&self) -> Option<&DurationHistogram> {
        self.scheduled_times_histogram.as_ref()
    }
}

impl Task {
    pub(crate) fn id(&self) -> Id<Task> {
        self.id
    }

    pub(crate) fn task_id(&self) -> TaskId {
        self.task_id.unwrap_or(0)
    }

    pub(crate) fn span_id(&self) -> SpanId {
        self.span_id
    }

    pub(crate) fn id_str(&self) -> &str {
        &self.id_str
    }

    pub(crate) fn target(&self) -> &str {
        &self.target
    }

    pub(crate) fn short_desc(&self) -> &str {
        &self.short_desc
    }

    pub(crate) fn name(&self) -> Option<&str> {
        self.name.as_ref().map(AsRef::as_ref)
    }

    pub(crate) fn formatted_fields(&self) -> &[Vec<Span<'static>>] {
        &self.formatted_fields
    }

    /// Returns `true` if this task is currently being polled.
    pub(crate) fn is_running(&self) -> bool {
        self.stats.last_poll_started > self.stats.last_poll_ended
    }

    pub(crate) fn is_scheduled(&self) -> bool {
        self.stats.last_wake > self.stats.last_poll_started
    }

    pub(crate) fn is_completed(&self) -> bool {
        self.stats.total.is_some()
    }

    pub(crate) fn state(&self) -> TaskState {
        if self.is_completed() {
            return TaskState::Completed;
        }

        if self.is_running() {
            return TaskState::Running;
        }

        if self.is_scheduled() {
            return TaskState::Scheduled;
        }

        TaskState::Idle
    }

    pub(crate) fn total(&self, since: SystemTime) -> Duration {
        self.stats
            .total
            .or_else(|| since.duration_since(self.stats.created_at).ok())
            .unwrap_or_default()
    }

    pub(crate) fn busy(&self, since: SystemTime) -> Duration {
        if let Some(started) = self.stats.last_poll_started {
            if self.stats.last_poll_started > self.stats.last_poll_ended {
                // in this case the task is being polled at the moment
                let current_time_in_poll = since.duration_since(started).unwrap_or_default();
                return self.stats.busy + current_time_in_poll;
            }
        }
        self.stats.busy
    }

    pub(crate) fn scheduled(&self, since: SystemTime) -> Duration {
        if let Some(wake) = self.stats.last_wake {
            if self.stats.last_wake > self.stats.last_poll_started {
                // In this case the task is scheduled, but has not yet been polled
                let current_time_since_wake = since.duration_since(wake).unwrap_or_default();
                return self.stats.scheduled + current_time_since_wake;
            }
        }
        self.stats.scheduled
    }

    pub(crate) fn idle(&self, since: SystemTime) -> Duration {
        self.stats
            .idle
            .or_else(|| {
                self.total(since)
                    .checked_sub(self.busy(since) + self.scheduled(since))
            })
            .unwrap_or_default()
    }

    /// Returns the total number of times the task has been polled.
    pub(crate) fn total_polls(&self) -> u64 {
        self.stats.polls
    }

    /// Returns the elapsed time since the task was last woken, relative to
    /// given `now` timestamp.
    ///
    /// Returns `None` if the task has never been woken, or if it was last woken
    /// more recently than `now` (which *shouldn't* happen as long as `now` is the
    /// timestamp of the last stats update...)
    pub(crate) fn since_wake(&self, now: SystemTime) -> Option<Duration> {
        now.duration_since(self.last_wake()?).ok()
    }

    pub(crate) fn last_wake(&self) -> Option<SystemTime> {
        self.stats.last_wake
    }

    /// Returns the current number of wakers for this task.
    pub(crate) fn waker_count(&self) -> u64 {
        self.waker_clones().saturating_sub(self.waker_drops())
    }

    /// Returns the total number of times this task's waker has been cloned.
    pub(crate) fn waker_clones(&self) -> u64 {
        self.stats.waker_clones
    }

    /// Returns the total number of times this task's waker has been dropped.
    pub(crate) fn waker_drops(&self) -> u64 {
        self.stats.waker_drops
    }

    /// Returns the total number of times this task has been woken.
    pub(crate) fn wakes(&self) -> u64 {
        self.stats.wakes
    }

    /// Returns the total number of times this task has woken itself.
    pub(crate) fn self_wakes(&self) -> u64 {
        self.stats.self_wakes
    }

    /// Returns the percentage of this task's total wakeups that were self-wakes.
    pub(crate) fn self_wake_percent(&self) -> u64 {
        self.self_wakes().percent_of(self.wakes())
    }

    /// Returns whether this task has signaled via its waker to run again.
    ///
    /// Once the task has been polled, this is changed back to false.
    pub(crate) fn is_awakened(&self) -> bool {
        // Before the first poll, the task is waiting on the executor to run it
        // for the first time.
        self.total_polls() == 0 || self.last_wake() > self.stats.last_poll_started
    }

    pub(crate) fn warnings(&self) -> &[Linter<Task>] {
        &self.warnings[..]
    }

    fn lint(&mut self, linters: &[Linter<Task>]) {
        self.warnings.clear();
        for lint in linters {
            tracing::debug!(?lint, task = ?self, "checking...");
            if let Some(warning) = lint.check(self) {
                tracing::info!(?warning, task = ?self, "found a warning!");
                self.warnings.push(warning)
            }
        }
    }

    pub(crate) fn location(&self) -> &str {
        &self.location
    }
}

impl From<proto::tasks::Stats> for TaskStats {
    fn from(pb: proto::tasks::Stats) -> Self {
        let created_at = pb
            .created_at
            .expect("task span was never created")
            .try_into()
            .unwrap();

        let dropped_at: Option<SystemTime> = pb.dropped_at.map(|v| v.try_into().unwrap());
        let total = dropped_at.map(|d| d.duration_since(created_at).unwrap_or_default());

        let poll_stats = pb.poll_stats.expect("task should have poll stats");
        let busy = poll_stats.busy_time.map(pb_duration).unwrap_or_default();
        let scheduled = pb.scheduled_time.map(pb_duration).unwrap_or_default();
        let idle = total.map(|total| total.checked_sub(busy + scheduled).unwrap_or_default());
        Self {
            total,
            idle,
            scheduled,
            busy,
            last_wake: pb.last_wake.map(|v| v.try_into().unwrap()),
            last_poll_started: poll_stats.last_poll_started.map(|v| v.try_into().unwrap()),
            last_poll_ended: poll_stats.last_poll_ended.map(|v| v.try_into().unwrap()),
            polls: poll_stats.polls,
            created_at,
            dropped_at,
            wakes: pb.wakes,
            waker_clones: pb.waker_clones,
            waker_drops: pb.waker_drops,
            self_wakes: pb.self_wakes,
        }
    }
}

impl Default for SortBy {
    fn default() -> Self {
        Self::Total
    }
}

impl SortBy {
    pub fn sort(&self, now: SystemTime, tasks: &mut [Weak<RefCell<Task>>]) {
        match self {
            Self::Tid => {
                tasks.sort_unstable_by_key(|task| task.upgrade().map(|t| t.borrow().task_id))
            }
            Self::Name => {
                tasks.sort_unstable_by_key(|task| task.upgrade().map(|t| t.borrow().name.clone()))
            }
            Self::State => {
                tasks.sort_unstable_by_key(|task| task.upgrade().map(|t| t.borrow().state()))
            }
            Self::Warns => tasks
                .sort_unstable_by_key(|task| task.upgrade().map(|t| t.borrow().warnings().len())),
            Self::Total => {
                tasks.sort_unstable_by_key(|task| task.upgrade().map(|t| t.borrow().total(now)))
            }
            Self::Idle => {
                tasks.sort_unstable_by_key(|task| task.upgrade().map(|t| t.borrow().idle(now)))
            }
            Self::Scheduled => {
                tasks.sort_unstable_by_key(|task| task.upgrade().map(|t| t.borrow().scheduled(now)))
            }
            Self::Busy => {
                tasks.sort_unstable_by_key(|task| task.upgrade().map(|t| t.borrow().busy(now)))
            }
            Self::Polls => {
                tasks.sort_unstable_by_key(|task| task.upgrade().map(|t| t.borrow().stats.polls))
            }
            Self::Target => {
                tasks.sort_unstable_by_key(|task| task.upgrade().map(|t| t.borrow().target.clone()))
            }
            Self::Location => tasks
                .sort_unstable_by_key(|task| task.upgrade().map(|t| t.borrow().location.clone())),
        }
    }
}

impl view::SortBy for SortBy {
    fn as_column(&self) -> usize {
        *self as usize
    }
}

impl TryFrom<usize> for SortBy {
    type Error = ();
    fn try_from(idx: usize) -> Result<Self, Self::Error> {
        match idx {
            idx if idx == Self::Tid as usize => Ok(Self::Tid),
            idx if idx == Self::State as usize => Ok(Self::State),
            idx if idx == Self::Warns as usize => Ok(Self::Warns),
            idx if idx == Self::Name as usize => Ok(Self::Name),
            idx if idx == Self::Total as usize => Ok(Self::Total),
            idx if idx == Self::Busy as usize => Ok(Self::Busy),
            idx if idx == Self::Scheduled as usize => Ok(Self::Scheduled),
            idx if idx == Self::Idle as usize => Ok(Self::Idle),
            idx if idx == Self::Polls as usize => Ok(Self::Polls),
            idx if idx == Self::Target as usize => Ok(Self::Target),
            idx if idx == Self::Location as usize => Ok(Self::Location),
            _ => Err(()),
        }
    }
}

impl TaskState {
    pub(crate) fn render(self, styles: &crate::view::Styles) -> Span<'static> {
        const RUNNING_UTF8: &str = "\u{25B6}";
        const SCHEDULED_UTF8: &str = "\u{23EB}";
        const IDLE_UTF8: &str = "\u{23F8}";
        const COMPLETED_UTF8: &str = "\u{23F9}";
        match self {
            Self::Running => Span::styled(
                styles.if_utf8(RUNNING_UTF8, "BUSY"),
                styles.fg(Color::Green),
            ),
            Self::Scheduled => Span::raw(styles.if_utf8(SCHEDULED_UTF8, "SCHED")),
            Self::Idle => Span::raw(styles.if_utf8(IDLE_UTF8, "IDLE")),
            Self::Completed => Span::raw(styles.if_utf8(COMPLETED_UTF8, "DONE")),
        }
    }
}
