use crate::view;
use console_api as proto;
use hdrhistogram::Histogram;
use std::{
    cell::RefCell,
    collections::HashMap,
    convert::TryFrom,
    fmt,
    io::Cursor,
    rc::{Rc, Weak},
    sync::Arc,
    time::{Duration, SystemTime},
};
use tui::text::Span;

#[derive(Default, Debug)]
pub(crate) struct State {
    tasks: HashMap<u64, Rc<RefCell<Task>>>,
    metas: HashMap<u64, Metadata>,
    last_updated_at: Option<SystemTime>,
    new_tasks: Vec<TaskRef>,
    current_task_details: DetailsRef,
}

#[derive(Debug, Copy, Clone)]
#[repr(usize)]
pub(crate) enum SortBy {
    Tid = 0,
    Total = 2,
    Busy = 3,
    Idle = 4,
    Polls = 5,
}

pub(crate) type TaskRef = Weak<RefCell<Task>>;
pub(crate) type DetailsRef = Rc<RefCell<Option<Details>>>;

#[derive(Debug)]
pub(crate) struct Task {
    id: u64,
    fields: Vec<Field>,
    formatted_fields: Vec<Vec<Span<'static>>>,
    kind: &'static str,
    stats: Stats,
    completed_for: usize,
    target: Arc<str>,
}

#[derive(Debug, Default)]
pub(crate) struct Details {
    task_id: u64,
    poll_times_histogram: Option<Histogram<u64>>,
    last_updated_at: Option<SystemTime>,
}

#[derive(Debug)]
pub(crate) struct Metadata {
    field_names: Vec<Arc<str>>,
    target: Arc<str>,
    //TODO: add more metadata as needed
}

#[derive(Debug)]
struct Stats {
    polls: u64,
    created_at: SystemTime,
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
}

#[derive(Debug)]
pub(crate) struct Field {
    pub(crate) name: Arc<str>,
    pub(crate) value: FieldValue,
}

#[derive(Debug)]
pub(crate) enum FieldValue {
    Bool(bool),
    Str(String),
    U64(u64),
    I64(i64),
    Debug(String),
}

impl State {
    const RETAIN_COMPLETED_FOR: usize = 6;

    pub(crate) fn len(&self) -> usize {
        self.tasks.len()
    }

    pub(crate) fn last_updated_at(&self) -> Option<SystemTime> {
        self.last_updated_at
    }

    /// Returns any new tasks that were added since the last task update.
    pub(crate) fn take_new_tasks(&mut self) -> impl Iterator<Item = TaskRef> + '_ {
        self.new_tasks.drain(..)
    }

    pub(crate) fn update_tasks(&mut self, update: proto::tasks::TaskUpdate) {
        if let Some(now) = update.now {
            self.last_updated_at = Some(now.into());
        }

        if let Some(new_metadata) = update.new_metadata {
            let metas = new_metadata.metadata.into_iter().filter_map(|meta| {
                let id = meta.id?.id;
                let metadata = meta.metadata?;
                Some((id, metadata.into()))
            });
            self.metas.extend(metas);
        }

        let mut stats_update = update.stats_update;
        let new_list = &mut self.new_tasks;
        new_list.clear();

        let metas = &mut self.metas;
        let new_tasks = update.new_tasks.into_iter().filter_map(|mut task| {
            let kind = match task.kind() {
                proto::tasks::task::Kind::Spawn => "T",
                proto::tasks::task::Kind::Blocking => "B",
            };

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
            let fields: Vec<Field> = task
                .fields
                .drain(..)
                .filter_map(|f| {
                    let field_name = f.name.as_ref()?;
                    let name: Option<Arc<str>> = match field_name {
                        proto::field::Name::StrName(n) => Some(n.clone().into()),
                        proto::field::Name::NameIdx(idx) => {
                            debug_assert_eq!(
                                f.metadata_id.map(|m| m.id),
                                Some(meta_id),
                                "malformed field name: metadata ID mismatch!"
                            );
                            meta.field_names.get(*idx as usize).cloned()
                        }
                    };
                    let value = f.value.as_ref().expect("no value").clone().into();
                    name.map(|name| Field { name, value })
                })
                .collect();

            let formatted_fields = fields.iter().fold(Vec::default(), |mut acc, f| {
                acc.push(vec![
                    view::bold(f.name.to_string()),
                    Span::from("="),
                    Span::from(format!("{} ", f.value)),
                ]);
                acc
            });

            let id = task.id;
            let stats = stats_update.remove(&id)?.into();
            let mut task = Task {
                id,
                fields,
                formatted_fields,
                kind,
                stats,
                completed_for: 0,
                target: meta.target.clone(),
            };
            task.update();
            let task = Rc::new(RefCell::new(task));
            new_list.push(Rc::downgrade(&task));
            Some((id, task))
        });
        self.tasks.extend(new_tasks);

        for (id, stats) in stats_update {
            if let Some(task) = self.tasks.get_mut(&id) {
                let mut t = task.borrow_mut();
                t.stats = stats.into();
                t.update();
            }
        }
    }

    pub(crate) fn details_ref(&self) -> DetailsRef {
        self.current_task_details.clone()
    }

    pub(crate) fn update_task_details(&mut self, update: proto::tasks::TaskDetails) {
        let details = Details {
            task_id: update.task_id,
            poll_times_histogram: update.poll_times_histogram.and_then(|data| {
                hdrhistogram::serialization::Deserializer::new()
                    .deserialize(&mut Cursor::new(&data))
                    .ok()
            }),
            last_updated_at: update.now.map(|now| now.into()),
        };

        *self.current_task_details.borrow_mut() = Some(details);
    }

    pub(crate) fn unset_task_details(&mut self) {
        *self.current_task_details.borrow_mut() = None;
    }

    pub(crate) fn retain_active(&mut self) {
        self.tasks.retain(|_, task| {
            let mut task = task.borrow_mut();
            if task.completed_for == 0 {
                return true;
            }
            task.completed_for += 1;
            task.completed_for <= Self::RETAIN_COMPLETED_FOR
        })
    }
}

impl Task {
    pub(crate) fn kind(&self) -> &str {
        self.kind
    }

    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    pub(crate) fn target(&self) -> &str {
        &self.target
    }

    pub(crate) fn formatted_fields(&self) -> &[Vec<Span<'static>>] {
        &self.formatted_fields
    }

    pub(crate) fn is_completed(&self) -> bool {
        self.stats.total.is_some()
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

    /// Returns the number of updates since the task completed
    pub(crate) fn completed_for(&self) -> usize {
        self.completed_for
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

    fn update(&mut self) {
        let completed = self.stats.total.is_some() && self.completed_for == 0;
        if completed {
            self.kind = "!";
            self.completed_for = 1;
        }
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

impl From<proto::tasks::Stats> for Stats {
    fn from(pb: proto::tasks::Stats) -> Self {
        fn pb_duration(dur: prost_types::Duration) -> Duration {
            let secs =
                u64::try_from(dur.seconds).expect("a task should not have a negative duration!");
            let nanos =
                u64::try_from(dur.nanos).expect("a task should not have a negative duration!");
            Duration::from_secs(secs) + Duration::from_nanos(nanos)
        }

        let total = pb.total_time.map(pb_duration);
        let busy = pb.busy_time.map(pb_duration).unwrap_or_default();
        let idle = total.map(|total| total - busy);
        Self {
            total,
            idle,
            busy,
            last_poll_started: pb.last_poll_started.map(Into::into),
            last_poll_ended: pb.last_poll_ended.map(Into::into),
            polls: pb.polls,
            created_at: pb.created_at.expect("task span was never created").into(),
            wakes: pb.wakes,
            waker_clones: pb.waker_clones,
            waker_drops: pb.waker_drops,
            last_wake: pb.last_wake.map(Into::into),
        }
    }
}

impl From<proto::Metadata> for Metadata {
    fn from(pb: proto::Metadata) -> Self {
        Self {
            field_names: pb.field_names.into_iter().map(|n| n.into()).collect(),
            target: pb.target.into(),
        }
    }
}

impl From<proto::field::Value> for FieldValue {
    fn from(pb: proto::field::Value) -> Self {
        match pb {
            proto::field::Value::BoolVal(v) => Self::Bool(v),
            proto::field::Value::StrVal(v) => Self::Str(v),
            proto::field::Value::I64Val(v) => Self::I64(v),
            proto::field::Value::U64Val(v) => Self::U64(v),
            proto::field::Value::DebugVal(v) => Self::Debug(v),
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
        // tasks.retain(|t| t.upgrade().is_some());
        match self {
            Self::Tid => tasks.sort_unstable_by_key(|task| task.upgrade().map(|t| t.borrow().id)),
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

impl TryFrom<usize> for SortBy {
    type Error = ();
    fn try_from(idx: usize) -> Result<Self, Self::Error> {
        match idx {
            idx if idx == Self::Tid as usize => Ok(Self::Tid),
            idx if idx == Self::Total as usize => Ok(Self::Total),
            idx if idx == Self::Busy as usize => Ok(Self::Busy),
            idx if idx == Self::Idle as usize => Ok(Self::Idle),
            idx if idx == Self::Polls as usize => Ok(Self::Polls),
            _ => Err(()),
        }
    }
}

impl fmt::Display for FieldValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FieldValue::Bool(v) => fmt::Display::fmt(v, f)?,
            FieldValue::Str(v) => fmt::Display::fmt(v, f)?,
            FieldValue::U64(v) => fmt::Display::fmt(v, f)?,
            FieldValue::Debug(v) => fmt::Display::fmt(v, f)?,
            FieldValue::I64(v) => fmt::Display::fmt(v, f)?,
        }

        Ok(())
    }
}
