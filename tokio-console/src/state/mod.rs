use self::{async_ops::AsyncOpsState, resources::ResourcesState};
use crate::{
    intern::{self, InternedStr},
    view,
    warnings::Linter,
};
use console_api as proto;
use ratatui::{
    style::{Color, Modifier},
    text::Span,
};
use std::{
    cell::RefCell,
    cmp::Ordering,
    collections::HashMap,
    convert::{TryFrom, TryInto},
    fmt,
    rc::Rc,
    time::{Duration, SystemTime},
};
use tasks::{Details, Task, TasksState};

pub mod async_ops;
pub mod histogram;
pub mod resources;
pub mod store;
pub mod tasks;

pub(crate) use self::store::Id;

pub(crate) type DetailsRef = Rc<RefCell<Option<Details>>>;

#[derive(Default, Debug)]
pub(crate) struct State {
    metas: HashMap<u64, Metadata>,
    last_updated_at: Option<SystemTime>,
    temporality: Temporality,
    tasks_state: TasksState,
    resources_state: ResourcesState,
    async_ops_state: AsyncOpsState,
    current_task_details: DetailsRef,
    retain_for: Option<Duration>,
    strings: intern::Strings,
}

pub(crate) enum Visibility {
    Show,
    Hide,
}

#[derive(Debug)]
pub(crate) struct Metadata {
    field_names: Vec<InternedStr>,
    target: InternedStr,
    id: u64,
    //TODO: add more metadata as needed
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Field {
    pub(crate) name: InternedStr,
    pub(crate) value: FieldValue,
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum FieldValue {
    Bool(bool),
    Str(String),
    U64(u64),
    I64(i64),
    Debug(String),
}

#[derive(Debug)]
enum Temporality {
    Live,
    Paused,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Attribute {
    field: Field,
    unit: Option<String>,
}

impl State {
    pub(crate) fn with_retain_for(mut self, retain_for: Option<Duration>) -> Self {
        self.retain_for = retain_for;
        self
    }

    pub(crate) fn with_task_linters(
        mut self,
        linters: impl IntoIterator<Item = Linter<Task>>,
    ) -> Self {
        self.tasks_state.linters.extend(linters);
        self
    }

    pub(crate) fn last_updated_at(&self) -> Option<SystemTime> {
        self.last_updated_at
    }

    pub(crate) fn update(
        &mut self,
        styles: &view::Styles,
        current_view: &view::ViewState,
        update: proto::instrument::Update,
    ) {
        if let Some(now) = update.now.map(|v| v.try_into().unwrap()) {
            self.last_updated_at = Some(now);
        }

        let strings = &mut self.strings;
        if let Some(new_metadata) = update.new_metadata {
            let metas = new_metadata.metadata.into_iter().filter_map(|meta| {
                let id = meta.id?.id;
                let metadata = meta.metadata?;
                Some((id, Metadata::from_proto(metadata, id, strings)))
            });
            self.metas.extend(metas);
        }

        if let Some(tasks_update) = update.task_update {
            let visibility = if matches!(current_view, view::ViewState::TasksList) {
                Visibility::Show
            } else {
                Visibility::Hide
            };
            self.tasks_state.update_tasks(
                styles,
                &mut self.strings,
                &self.metas,
                tasks_update,
                visibility,
            )
        }

        if let Some(resources_update) = update.resource_update {
            let visibility = if matches!(current_view, view::ViewState::ResourcesList) {
                Visibility::Show
            } else {
                Visibility::Hide
            };
            self.resources_state.update_resources(
                styles,
                &mut self.strings,
                &self.metas,
                resources_update,
                visibility,
            )
        }

        if let Some(async_ops_update) = update.async_op_update {
            let visibility = if matches!(current_view, view::ViewState::ResourceInstance(_)) {
                Visibility::Show
            } else {
                Visibility::Hide
            };
            self.async_ops_state.update_async_ops(
                styles,
                &mut self.strings,
                &self.metas,
                async_ops_update,
                self.resources_state.ids_mut(),
                self.tasks_state.ids_mut(),
                visibility,
            )
        }
    }

    pub(crate) fn retain_active(&mut self) {
        if self.is_paused() {
            return;
        }

        if let (Some(now), Some(retain_for)) = (self.last_updated_at(), self.retain_for) {
            self.tasks_state.retain_active(now, retain_for);
            self.resources_state.retain_active(now, retain_for);
            self.async_ops_state.retain_active(now, retain_for);
        }

        // After dropping idle tasks & resources, prune any interned strings
        // that are no longer referenced.
        self.strings.retain_referenced();
    }

    pub(crate) fn task_details_ref(&self) -> DetailsRef {
        self.current_task_details.clone()
    }

    pub(crate) fn tasks_state(&mut self) -> &TasksState {
        &self.tasks_state
    }

    pub(crate) fn tasks_state_mut(&mut self) -> &mut TasksState {
        &mut self.tasks_state
    }

    pub(crate) fn resources_state(&mut self) -> &ResourcesState {
        &self.resources_state
    }

    pub(crate) fn resources_state_mut(&mut self) -> &mut ResourcesState {
        &mut self.resources_state
    }

    pub(crate) fn async_ops_state(&self) -> &AsyncOpsState {
        &self.async_ops_state
    }

    pub(crate) fn async_ops_state_mut(&mut self) -> &mut AsyncOpsState {
        &mut self.async_ops_state
    }

    pub(crate) fn update_task_details(&mut self, update: proto::tasks::TaskDetails) {
        if let Some(id) = update.task_id {
            let details = Details {
                span_id: id.id,
                poll_times_histogram: update
                    .poll_times_histogram
                    .as_ref()
                    .and_then(histogram::DurationHistogram::from_poll_durations),
                scheduled_times_histogram: update
                    .scheduled_times_histogram
                    .as_ref()
                    .and_then(histogram::DurationHistogram::from_proto),
            };

            *self.current_task_details.borrow_mut() = Some(details);
        }
    }

    pub(crate) fn unset_task_details(&mut self) {
        *self.current_task_details.borrow_mut() = None;
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
}

impl Default for Temporality {
    fn default() -> Self {
        Self::Live
    }
}

impl Metadata {
    fn from_proto(pb: proto::Metadata, id: u64, strings: &mut intern::Strings) -> Self {
        Self {
            field_names: pb
                .field_names
                .into_iter()
                .map(|n| strings.string(n))
                .collect(),
            target: strings.string(pb.target),
            id,
        }
    }
}

// === impl Field ===

impl Field {
    const SPAWN_LOCATION: &'static str = "spawn.location";
    const KIND: &'static str = "kind";
    const NAME: &'static str = "task.name";
    const TASK_ID: &'static str = "task.id";

    /// Creates a new Field with a pre-interned `name` and a `FieldValue`.
    fn new(name: InternedStr, value: FieldValue) -> Self {
        Field { name, value }
    }

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
        strings: &mut intern::Strings,
    ) -> Option<Self> {
        use proto::field::Name;
        let name = match name? {
            Name::StrName(n) => strings.string(n),
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

    fn make_formatted(styles: &view::Styles, fields: &mut [Field]) -> Vec<Vec<Span<'static>>> {
        let key_style = styles.fg(Color::LightBlue).add_modifier(Modifier::BOLD);
        let delim_style = styles.fg(Color::LightBlue).add_modifier(Modifier::DIM);
        let val_style = styles.fg(Color::Yellow);

        fields.sort_unstable();

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

impl Ord for Field {
    fn cmp(&self, other: &Self) -> Ordering {
        match (&*self.name, &*other.name) {
            // the `NAME` field should always come first
            (Field::NAME, Field::NAME) => Ordering::Equal,
            (Field::NAME, _) => Ordering::Less,
            (_, Field::NAME) => Ordering::Greater,

            // the `SPAWN_LOCATION` field should always come last (it's long)
            (Field::SPAWN_LOCATION, Field::SPAWN_LOCATION) => Ordering::Equal,
            (Field::SPAWN_LOCATION, _) => Ordering::Greater,
            (_, Field::SPAWN_LOCATION) => Ordering::Less,
            (this, that) => this.cmp(that),
        }
    }
}

impl PartialOrd for Field {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
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
        match self {
            FieldValue::Str(s) | FieldValue::Debug(s) => {
                FieldValue::Debug(truncate_registry_path(s))
            }

            f => f,
        }
    }

    /// If `self` is an empty string, returns `None`. Otherwise, returns `Some(self)`.
    fn ensure_nonempty(self) -> Option<Self> {
        match self {
            FieldValue::Debug(s) | FieldValue::Str(s) if s.is_empty() => None,
            val => Some(val),
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

impl Ord for Attribute {
    fn cmp(&self, other: &Self) -> Ordering {
        self.field
            .cmp(&other.field)
            // TODO(eliza): *maybe* this should compare so that larger units are
            // greater than smaller units (e.g. `ms` > `us`), rather than
            // alphabetically?
            // but, meh...
            .then_with(|| self.unit.cmp(&other.unit))
    }
}

impl PartialOrd for Attribute {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// === impl Attribute ===

impl Attribute {
    fn make_formatted(
        styles: &view::Styles,
        attributes: &mut [Attribute],
    ) -> Vec<Vec<Span<'static>>> {
        let key_style = styles.fg(Color::LightBlue).add_modifier(Modifier::BOLD);
        let delim_style = styles.fg(Color::LightBlue).add_modifier(Modifier::DIM);
        let val_style = styles.fg(Color::Yellow);
        let unit_style = styles.fg(Color::LightBlue);

        attributes.sort_unstable();

        let mut formatted = Vec::with_capacity(attributes.len());
        let attributes = attributes.iter();
        for attr in attributes {
            let mut elems = vec![
                Span::styled(attr.field.name.to_string(), key_style),
                Span::styled("=", delim_style),
                Span::styled(format!("{}", attr.field.value), val_style),
            ];

            if let Some(unit) = &attr.unit {
                elems.push(Span::styled(unit.clone(), unit_style))
            }
            elems.push(Span::raw(" "));
            formatted.push(elems)
        }
        formatted
    }
}

fn truncate_registry_path(s: String) -> String {
    use once_cell::sync::OnceCell;
    use regex::Regex;
    use std::borrow::Cow;

    static REGEX: OnceCell<Regex> = OnceCell::new();
    let regex = REGEX.get_or_init(|| {
        Regex::new(
            r".*(/|\\)\.cargo(/|\\)(registry(/|\\)src(/|\\)[^/\\]*(/|\\)|git(/|\\)checkouts(/|\\))",
        )
        .expect("failed to compile regex")
    });

    let s = match regex.replace(&s, "<cargo>/") {
        Cow::Owned(s) => s,
        // String was not modified, return the original.
        Cow::Borrowed(_) => s,
    };

    // This help use the same path separator on all platforms.
    s.replace('\\', "/")
}

fn format_location(loc: Option<proto::Location>) -> String {
    loc.map(|mut l| {
        if let Some(file) = l.file.take() {
            let truncated = truncate_registry_path(file);
            l.file = Some(truncated);
        }
        l.to_string()
    })
    .unwrap_or_else(|| "<unknown location>".to_string())
}

fn pb_duration(dur: prost_types::Duration) -> Duration {
    let secs = u64::try_from(dur.seconds).expect("duration should not be negative!");
    let nanos = u64::try_from(dur.nanos).expect("duration should not be negative!");
    Duration::from_secs(secs) + Duration::from_nanos(nanos)
}

#[cfg(test)]
mod tests {
    use super::*;

    // This test should be run on all platforms. The console can display instrumentation data
    // from different console subscribers that may be running on different operating systems.
    // For instance, the console could be running on Windows, while the application is running on Linux.
    // Therefore, it's important to ensure that paths, which can differ between operating systems,
    // are displayed correctly in the console.
    #[test]
    fn test_format_location_linux() {
        // Linux style paths.
        let location1 = proto::Location {
            file: Some(
                "/home/user/.cargo/registry/src/github.com-1ecc6299db9ec823/tokio-1.0.1/src/lib.rs"
                    .to_string(),
            ),
            ..Default::default()
        };
        let location2 = proto::Location {
            file: Some("/home/user/.cargo/git/checkouts/tokio-1.0.1/src/lib.rs".to_string()),
            ..Default::default()
        };
        let location3 = proto::Location {
            file: Some("/home/user/projects/tokio-1.0.1/src/lib.rs".to_string()),
            ..Default::default()
        };

        assert_eq!(
            format_location(Some(location1)),
            "<cargo>/tokio-1.0.1/src/lib.rs"
        );
        assert_eq!(
            format_location(Some(location2)),
            "<cargo>/tokio-1.0.1/src/lib.rs"
        );
        assert_eq!(
            format_location(Some(location3)),
            "/home/user/projects/tokio-1.0.1/src/lib.rs"
        );

        assert_eq!(format_location(None), "<unknown location>");
    }

    #[test]
    fn test_format_location_macos() {
        // macOS style paths.
        let location1 = proto::Location {
            file: Some("/Users/user/.cargo/registry/src/github.com-1ecc6299db9ec823/tokio-1.0.1/src/lib.rs".to_string()),
            ..Default::default()
        };
        let location2 = proto::Location {
            file: Some("/Users/user/.cargo/git/checkouts/tokio-1.0.1/src/lib.rs".to_string()),
            ..Default::default()
        };
        let location3 = proto::Location {
            file: Some("/Users/user/projects/tokio-1.0.1/src/lib.rs".to_string()),
            ..Default::default()
        };

        assert_eq!(
            format_location(Some(location1)),
            "<cargo>/tokio-1.0.1/src/lib.rs"
        );
        assert_eq!(
            format_location(Some(location2)),
            "<cargo>/tokio-1.0.1/src/lib.rs"
        );
        assert_eq!(
            format_location(Some(location3)),
            "/Users/user/projects/tokio-1.0.1/src/lib.rs"
        );
    }

    #[test]
    fn test_format_location_windows() {
        // Windows style paths.
        let location1 = proto::Location {
            file: Some(
                "C:\\Users\\user\\.cargo\\registry\\src\\github.com-1ecc6299db9ec823\\tokio-1.0.1\\src\\lib.rs"
                    .to_string(),
            ),
            ..Default::default()
        };

        let location2 = proto::Location {
            file: Some(
                "C:\\Users\\user\\.cargo\\git\\checkouts\\tokio-1.0.1\\src\\lib.rs".to_string(),
            ),
            ..Default::default()
        };

        let location3 = proto::Location {
            file: Some("C:\\Users\\user\\projects\\tokio-1.0.1\\src\\lib.rs".to_string()),
            ..Default::default()
        };

        assert_eq!(
            format_location(Some(location1)),
            "<cargo>/tokio-1.0.1/src/lib.rs"
        );

        assert_eq!(
            format_location(Some(location2)),
            "<cargo>/tokio-1.0.1/src/lib.rs"
        );

        assert_eq!(
            format_location(Some(location3)),
            "C:/Users/user/projects/tokio-1.0.1/src/lib.rs"
        );
    }
}
