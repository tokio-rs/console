use console_api as proto;
use std::collections::HashMap;
use std::time::Duration;
use tui::{
    layout,
    style::{self, Style},
    widgets::{Block, Cell, Row, Table, TableState},
};

#[derive(Default, Debug)]
pub(crate) struct State {
    tasks: HashMap<u64, Task>,
    table_state: TableState,
}

#[derive(Default, Debug)]
struct Task {
    id_hex: String,
    fields: String,
    kind: &'static str,
    stats: Stats,
}

#[derive(Default, Debug)]
struct Stats {
    polls: u64,
    busy: Duration,
    idle: Duration,
    total: Duration,
}
impl State {
    pub(crate) fn len(&self) -> usize {
        self.tasks.len()
    }
    pub(crate) fn update(&mut self, update: proto::tasks::TaskUpdate) {
        let new_tasks = update.new_tasks.into_iter().filter_map(|task| {
            if task.id.is_none() {
                tracing::warn!(?task, "skipping task with no id");
            }
            let kind = match task.kind() {
                proto::tasks::task::Kind::Spawn => "T",
                proto::tasks::task::Kind::Blocking => "B",
            };

            let id = task.id?.id;
            let task = Task {
                id_hex: format!("{:x}", id),
                fields: task.string_fields,
                kind,
                stats: Default::default(),
            };
            Some((id, task))
        });
        self.tasks.extend(new_tasks);

        for (id, stats) in update.stats_update {
            if let Some(task) = self.tasks.get_mut(&id) {
                task.stats = stats.into();
            }
        }

        for proto::SpanId { id } in update.completed {
            if self.tasks.remove(&id).is_none() {
                tracing::warn!(?id, "tried to complete a task that didn't exist");
            }
        }
    }

    pub(crate) fn render<B: tui::backend::Backend>(
        &mut self,
        frame: &mut tui::terminal::Frame<B>,
        area: layout::Rect,
    ) {
        const HEADER: &[&str] = &["TID", "KIND", "TOTAL", "BUSY", "IDLE", "POLLS", "FIELDS"];
        const DUR_LEN: usize = 10;
        // This data is only updated every second, so it doesn't make a ton of
        // sense to have a lot of precision in timestamps (and this makes sure
        // there's room for the unit!)
        const DUR_PRECISION: usize = 4;
        const POLLS_LEN: usize = 5;
        let rows = self.tasks.values().map(|task| {
            let row = Row::new(vec![
                Cell::from(task.id_hex.as_str()),
                // TODO(eliza): is there a way to write a `fmt::Debug` impl
                // directly to tui without doing an allocation?
                Cell::from(task.kind),
                Cell::from(format!(
                    "{:>width$.prec$?}",
                    task.stats.total,
                    width = DUR_LEN,
                    prec = DUR_PRECISION,
                )),
                Cell::from(format!(
                    "{:>width$.prec$?}",
                    task.stats.busy,
                    width = DUR_LEN,
                    prec = DUR_PRECISION,
                )),
                Cell::from(format!(
                    "{:>width$.prec$?}",
                    task.stats.idle,
                    width = DUR_LEN,
                    prec = DUR_PRECISION,
                )),
                Cell::from(format!("{:>width$}", task.stats.polls, width = POLLS_LEN)),
                Cell::from(task.fields.as_str()),
            ]);
            row
        });
        let t = Table::new(rows)
            .header(
                Row::new(HEADER.iter().map(|&v| Cell::from(v)))
                    .height(1)
                    .style(Style::default().add_modifier(style::Modifier::REVERSED)),
            )
            .block(Block::default())
            .widths(&[
                layout::Constraint::Min(20),
                layout::Constraint::Length(4),
                layout::Constraint::Min(DUR_LEN as u16),
                layout::Constraint::Min(DUR_LEN as u16),
                layout::Constraint::Min(DUR_LEN as u16),
                layout::Constraint::Min(POLLS_LEN as u16),
                layout::Constraint::Min(10),
            ]);

        frame.render_widget(t, area)
    }
}

impl From<proto::tasks::Stats> for Stats {
    fn from(pb: proto::tasks::Stats) -> Self {
        fn pb_duration(dur: prost_types::Duration) -> Duration {
            use std::convert::TryFrom;

            let secs =
                u64::try_from(dur.seconds).expect("a task should not have a negative duration!");
            let nanos =
                u64::try_from(dur.nanos).expect("a task should not have a negative duration!");
            Duration::from_secs(secs) + Duration::from_nanos(nanos)
        }

        let total = pb.total_time.map(pb_duration).unwrap_or_default();
        let busy = pb.busy_time.map(pb_duration).unwrap_or_default();
        let idle = total - busy;
        Self {
            total,
            idle,
            busy,
            polls: pb.polls,
        }
    }
}
