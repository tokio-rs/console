use crate::{util::Percentage, view, warnings::Linter};
use console_api as proto;
use hdrhistogram::Histogram;
use std::{
    cell::RefCell,
    collections::HashMap,
    convert::{TryFrom, TryInto},
    fmt,
    io::Cursor,
    rc::{Rc, Weak},
    sync::Arc,
    time::{Duration, SystemTime},
};
use tui::{
    style::{Color, Modifier},
    text::Span,
};

#[derive(Default, Debug)]
pub(crate) struct State {
    tasks: HashMap<u64, Rc<RefCell<Task>>>,
    metas: HashMap<u64, Metadata>,
    linters: Vec<Linter<Task>>,
    last_updated_at: Option<SystemTime>,
    new_tasks: Vec<TaskRef>,
    current_task_details: DetailsRef,
    temporality: Temporality,
    retain_for: Option<Duration>,
    wake_to_poll_times_histogram: Option<Histogram<u64>>,
}

#[derive(Debug)]
enum Temporality {
    Live,
    Paused,
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
pub(crate) type DetailsRef = Rc<RefCell<Option<Details>>>;

#[derive(Debug)]
pub(crate) struct Task {
    id: u64,
    fields: Vec<Field>,
    formatted_fields: Vec<Vec<Span<'static>>>,
    stats: Stats,
    target: Arc<str>,
    name: Option<Arc<str>>,
    /// Currently active warnings for this task.
    warnings: Vec<Linter<Task>>,
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
    id: u64,
    //TODO: add more metadata as needed
}

#[derive(Debug)]
struct Stats {
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
    pub(crate) fn with_retain_for(mut self, retain_for: Option<Duration>) -> Self {
        self.retain_for = retain_for;
        self
    }

    pub(crate) fn with_linters(mut self, linters: impl IntoIterator<Item = Linter<Task>>) -> Self {
        self.linters.extend(linters.into_iter());
        self
    }

    pub(crate) fn last_updated_at(&self) -> Option<SystemTime> {
        self.last_updated_at
    }

    /// Returns any new tasks that were added since the last task update.
    pub(crate) fn take_new_tasks(&mut self) -> impl Iterator<Item = TaskRef> + '_ {
        self.new_tasks.drain(..)
    }

    pub(crate) fn update_tasks(
        &mut self,
        styles: &view::Styles,
        update: proto::tasks::TaskUpdate,
        new_metadata: Option<proto::RegisterMetadata>,
        now: Option<SystemTime>,
    ) {
        if let Some(now) = now {
            self.last_updated_at = Some(now);
        }

        if let Some(new_metadata) = new_metadata {
            let metas = new_metadata.metadata.into_iter().filter_map(|meta| {
                let id = meta.id?.id;
                let metadata = meta.metadata?;
                Some((id, Metadata::from_proto(metadata, id)))
            });
            self.metas.extend(metas);
        }

        let mut stats_update = update.stats_update;
        let new_list = &mut self.new_tasks;
        new_list.clear();
        let linters = &self.linters;

        let metas = &mut self.metas;
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

    pub(crate) fn details_ref(&self) -> DetailsRef {
        self.current_task_details.clone()
    }

    pub(crate) fn update_task_details(&mut self, update: proto::tasks::TaskDetails) {
        if let Some(id) = update.task_id {
            let details = Details {
                task_id: id.id,
                poll_times_histogram: update.poll_times_histogram.and_then(|data| {
                    hdrhistogram::serialization::Deserializer::new()
                        .deserialize(&mut Cursor::new(&data))
                        .ok()
                }),
                last_updated_at: update.now.map(|now| now.try_into().unwrap()),
            };

            *self.current_task_details.borrow_mut() = Some(details);
        }
    }

    pub(crate) fn wake_to_poll_times_histogram_ref(&self) -> Option<&Histogram<u64>> {
        self.wake_to_poll_times_histogram.as_ref()
    }

    pub(crate) fn update_wake_to_poll_histogram(&mut self, data: Vec<u8>) {
        self.wake_to_poll_times_histogram = hdrhistogram::serialization::Deserializer::new()
            .deserialize(&mut Cursor::new(&data))
            .ok();
    }

    pub(crate) fn unset_task_details(&mut self) {
        *self.current_task_details.borrow_mut() = None;
    }

    pub(crate) fn retain_active(&mut self) {
        // Don't clean up stopped tasks while the console is paused.
        if self.is_paused() {
            return;
        }

        if let (Some(now), Some(retain_for)) = (self.last_updated_at, self.retain_for) {
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
    }

    // temporality methods

    pub(crate) fn pause(&mut self) {
        self.temporality = Temporality::Paused;
    }

    pub(crate) fn resume(&mut self) {
        self.temporality = Temporality::Live;
    }

    pub(crate) fn is_paused(&self) -> bool {
        matches!(self.temporality, Temporality::Paused)
    }

    pub(crate) fn warnings(&self) -> impl Iterator<Item = &Linter<Task>> {
        self.linters.iter().filter(|linter| linter.count() > 0)
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

impl Metadata {
    fn from_proto(pb: proto::Metadata, id: u64) -> Self {
        Self {
            field_names: pb.field_names.into_iter().map(|n| n.into()).collect(),
            target: pb.target.into(),
            id,
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

impl Default for Temporality {
    fn default() -> Self {
        Self::Live
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

// === impl Field ===

impl Field {
    const SPAWN_LOCATION: &'static str = "spawn.location";
    const NAME: &'static str = "task.name";

    /// Converts a wire-format `Field` into an internal `Field` representation,
    /// using the provided `Metadata` for the task span that the field came
    /// from.
    ///
    /// If the field is invalid or it has a string value which is empty, this
    /// returns `None`.
    fn from_proto(
        proto::Field {
            name,
            metadata_id,
            value,
        }: proto::Field,
        meta: &Metadata,
    ) -> Option<Self> {
        use proto::field::Name;
        let name: Arc<str> = match name? {
            Name::StrName(n) => n.into(),
            Name::NameIdx(idx) => {
                let meta_id = metadata_id.map(|m| m.id);
                if meta_id != Some(meta.id) {
                    tracing::warn!(
                        task.meta_id = meta.id,
                        field.meta.id = ?meta_id,
                        field.name_index = idx,
                        ?meta,
                        "skipping malformed field name (metadata id mismatch)"
                    );
                    debug_assert_eq!(
                        meta_id,
                        Some(meta.id),
                        "malformed field name: metadata ID mismatch! (name idx={}; metadata={:#?})",
                        idx,
                        meta,
                    );
                    return None;
                }
                match meta.field_names.get(idx as usize).cloned() {
                    Some(name) => name,
                    None => {
                        tracing::warn!(
                            task.meta_id = meta.id,
                            field.meta.id = ?meta_id,
                            field.name_index = idx,
                            ?meta,
                            "missing field name for index"
                        );
                        return None;
                    }
                }
            }
        };

        debug_assert!(
            value.is_some(),
            "missing field value for field `{:?}` (metadata={:#?})",
            name,
            meta,
        );
        let mut value = FieldValue::from(value?)
            // if the value is an empty string, just skip it.
            .ensure_nonempty()?;

        if &*name == Field::SPAWN_LOCATION {
            value = value.truncate_registry_path();
        }

        Some(Self { name, value })
    }

    fn make_formatted(styles: &view::Styles, fields: &mut Vec<Field>) -> Vec<Vec<Span<'static>>> {
        use std::cmp::Ordering;

        let key_style = styles.fg(Color::LightBlue).add_modifier(Modifier::BOLD);
        let delim_style = styles.fg(Color::LightBlue).add_modifier(Modifier::DIM);
        let val_style = styles.fg(Color::Yellow);

        fields.sort_unstable_by(|left, right| {
            if &*left.name == Field::NAME {
                return Ordering::Less;
            }

            if &*right.name == Field::NAME {
                return Ordering::Greater;
            }

            if &*left.name == Field::SPAWN_LOCATION {
                return Ordering::Greater;
            }

            if &*right.name == Field::SPAWN_LOCATION {
                return Ordering::Less;
            }

            left.name.cmp(&right.name)
        });

        let mut formatted = Vec::with_capacity(fields.len());
        let mut fields = fields.iter();
        if let Some(field) = fields.next() {
            formatted.push(vec![
                Span::styled(field.name.to_string(), key_style),
                Span::styled("=", delim_style),
                Span::styled(format!("{} ", field.value), val_style),
            ]);
            for field in fields {
                formatted.push(vec![
                    // Span::styled(", ", delim_style),
                    Span::styled(field.name.to_string(), key_style),
                    Span::styled("=", delim_style),
                    Span::styled(format!("{} ", field.value), val_style),
                ])
            }
        }
        formatted
    }
}

// === impl FieldValue ===

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

impl FieldValue {
    /// Truncates paths including `.cargo/registry`.
    fn truncate_registry_path(self) -> Self {
        use once_cell::sync::OnceCell;
        use regex::Regex;
        use std::borrow::Cow;

        static REGEX: OnceCell<Regex> = OnceCell::new();
        let regex = REGEX.get_or_init(|| {
            Regex::new(r#".*/\.cargo/registry/src/[^/]*/"#).expect("failed to compile regex")
        });

        let s = match self {
            FieldValue::Str(s) | FieldValue::Debug(s) => s,
            f => return f,
        };

        let s = match regex.replace(&s, "<cargo>/") {
            Cow::Owned(s) => s,
            // String was not modified, return the original.
            Cow::Borrowed(_) => s,
        };
        FieldValue::Debug(s)
    }

    /// If `self` is an empty string, returns `None`. Otherwise, returns `Some(self)`.
    fn ensure_nonempty(self) -> Option<Self> {
        match self {
            FieldValue::Debug(s) | FieldValue::Str(s) if s.is_empty() => None,
            val => Some(val),
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
