use crate::state::{Field, Metadata, Visibility};
use crate::{util::Percentage, view, warnings::Linter};
use console_api as proto;
use hdrhistogram::Histogram;
use std::{
    cell::RefCell,
    collections::HashMap,
    convert::{TryFrom, TryInto},
    rc::{Rc, Weak},
    sync::Arc,
    time::{Duration, SystemTime},
};
use tui::{style::Color, text::Span};

#[derive(Default, Debug)]
pub(crate) struct TasksState {
    tasks: HashMap<u64, Rc<RefCell<Task>>>,
    new_tasks: Vec<TaskRef>,
    pub(crate) linters: Vec<Linter<Task>>,
}

#[derive(Debug, Default)]
pub(crate) struct Details {
    pub(crate) task_id: u64,
    pub(crate) poll_times_histogram: Option<Histogram<u64>>,
    pub(crate) last_updated_at: Option<SystemTime>,
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
    Idle = 6,
    Polls = 7,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum TaskState {
    Completed,
    Idle,
    Running,
}

pub(crate) type TaskRef = Weak<RefCell<Task>>;

#[derive(Debug)]
pub(crate) struct Task {
    id: u64,
    fields: Vec<Field>,
    formatted_fields: Vec<Vec<Span<'static>>>,
    stats: TaskStats,
    target: Arc<str>,
    name: Option<Arc<str>>,
    /// Currently active warnings for this task.
    warnings: Vec<Linter<Task>>,
}

#[derive(Debug)]
struct TaskStats {
    polls: u64,
    created_at: SystemTime,
    dropped_at: Option<SystemTime>,
    busy: Duration,
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
        self.new_tasks.drain(..)
    }

    pub(crate) fn update_tasks(
        &mut self,
        styles: &view::Styles,
        update: proto::tasks::TaskUpdate,
        metas: &HashMap<u64, Metadata>,
        visibility: Visibility,
    ) {
        let mut stats_update = update.stats_update;
        let new_list = &mut self.new_tasks;
        if matches!(visibility, Visibility::Show) {
            new_list.clear();
        }

        let linters = &self.linters;

        let new_tasks = update.new_tasks.into_iter().filter_map(|mut task| {
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
            let mut fields = task
                .fields
                .drain(..)
                .filter_map(|pb| {
                    let field = Field::from_proto(pb, meta)?;
                    // the `task.name` field gets its own column, if it's present.
                    if &*field.name == Field::NAME {
                        name = Some(field.value.to_string().into());
                        return None;
                    }
                    Some(field)
                })
                .collect::<Vec<_>>();

            let formatted_fields = Field::make_formatted(styles, &mut fields);
            let id = task.id?.id;
            let stats = stats_update.remove(&id)?.into();
            let mut task = Task {
                name,
                id,
                fields,
                formatted_fields,
                stats,
                target: meta.target.clone(),
                warnings: Vec::new(),
            };
            task.lint(linters);
            let task = Rc::new(RefCell::new(task));
            new_list.push(Rc::downgrade(&task));
            Some((id, task))
        });
        self.tasks.extend(new_tasks);
        for (id, stats) in stats_update {
            if let Some(task) = self.tasks.get_mut(&id) {
                let mut task = task.borrow_mut();
                tracing::trace!(?task, "processing stats update for");
                task.stats = stats.into();
                task.lint(linters);
            }
        }
    }

    pub(crate) fn retain_active(&mut self, now: SystemTime, retain_for: Duration) {
        self.tasks.retain(|_, task| {
            let task = task.borrow();

            task.stats
                .dropped_at
                .map(|d| {
                    let dropped_for = now.duration_since(d).unwrap();
                    retain_for > dropped_for
                })
                .unwrap_or(true)
        })
    }

    pub(crate) fn warnings(&self) -> impl Iterator<Item = &Linter<Task>> {
        self.linters.iter().filter(|linter| linter.count() > 0)
    }
}

impl Details {
    pub(crate) fn task_id(&self) -> u64 {
        self.task_id
    }

    pub(crate) fn poll_times_histogram(&self) -> Option<&Histogram<u64>> {
        self.poll_times_histogram.as_ref()
    }
}

impl Task {
    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    pub(crate) fn target(&self) -> &str {
        &self.target
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

        TaskState::Idle
    }

    pub(crate) fn total(&self, since: SystemTime) -> Duration {
        self.stats
            .total
            .unwrap_or_else(|| since.duration_since(self.stats.created_at).unwrap())
    }

    pub(crate) fn busy(&self, since: SystemTime) -> Duration {
        if let (Some(last_poll_started), None) =
            (self.stats.last_poll_started, self.stats.last_poll_ended)
        {
            // in this case the task is being polled at the moment
            let current_time_in_poll = since.duration_since(last_poll_started).unwrap();
            return self.stats.busy + current_time_in_poll;
        }
        self.stats.busy
    }

    pub(crate) fn idle(&self, since: SystemTime) -> Duration {
        self.stats
            .idle
            .unwrap_or_else(|| self.total(since) - self.busy(since))
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
        self.last_wake() > self.stats.last_poll_started
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
}

impl From<proto::tasks::Stats> for TaskStats {
    fn from(pb: proto::tasks::Stats) -> Self {
        fn pb_duration(dur: prost_types::Duration) -> Duration {
            let secs =
                u64::try_from(dur.seconds).expect("a task should not have a negative duration!");
            let nanos =
                u64::try_from(dur.nanos).expect("a task should not have a negative duration!");
            Duration::from_secs(secs) + Duration::from_nanos(nanos)
        }

        let created_at = pb
            .created_at
            .expect("task span was never created")
            .try_into()
            .unwrap();

        let dropped_at: Option<SystemTime> = pb.dropped_at.map(|v| v.try_into().unwrap());
        let total = dropped_at.map(|d| d.duration_since(created_at).unwrap());

        let poll_stats = pb.poll_stats.expect("task should have poll stats");
        let busy = poll_stats.busy_time.map(pb_duration).unwrap_or_default();
        let idle = total.map(|total| total - busy);
        Self {
            total,
            idle,
            busy,
            last_poll_started: poll_stats.last_poll_started.map(|v| v.try_into().unwrap()),
            last_poll_ended: poll_stats.last_poll_ended.map(|v| v.try_into().unwrap()),
            polls: poll_stats.polls,
            created_at,
            dropped_at,
            wakes: pb.wakes,
            waker_clones: pb.waker_clones,
            waker_drops: pb.waker_drops,
            last_wake: pb.last_wake.map(|v| v.try_into().unwrap()),
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
    pub fn sort(&self, now: SystemTime, tasks: &mut Vec<Weak<RefCell<Task>>>) {
        match self {
            Self::Tid => tasks.sort_unstable_by_key(|task| task.upgrade().map(|t| t.borrow().id)),
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
            Self::Busy => {
                tasks.sort_unstable_by_key(|task| task.upgrade().map(|t| t.borrow().busy(now)))
            }
            Self::Polls => {
                tasks.sort_unstable_by_key(|task| task.upgrade().map(|t| t.borrow().stats.polls))
            }
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
            idx if idx == Self::Idle as usize => Ok(Self::Idle),
            idx if idx == Self::Polls as usize => Ok(Self::Polls),
            _ => Err(()),
        }
    }
}

impl TaskState {
    pub(crate) fn render(self, styles: &crate::view::Styles) -> Span<'static> {
        const RUNNING_UTF8: &str = "\u{25B6}";
        const IDLE_UTF8: &str = "\u{23F8}";
        const COMPLETED_UTF8: &str = "\u{23F9}";
        match self {
            Self::Running => Span::styled(
                styles.if_utf8(RUNNING_UTF8, "BUSY"),
                styles.fg(Color::Green),
            ),
            Self::Idle => Span::raw(styles.if_utf8(IDLE_UTF8, "IDLE")),
            Self::Completed => Span::raw(styles.if_utf8(COMPLETED_UTF8, "DONE")),
        }
    }
}
