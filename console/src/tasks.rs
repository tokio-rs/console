use crate::input;
use console_api as proto;
use std::{
    cell::RefCell,
    collections::HashMap,
    convert::TryFrom,
    rc::{Rc, Weak},
    time::{Duration, SystemTime},
};
use tui::{
    layout,
    style::{self, Style},
    text,
    widgets::{Block, Cell, Row, Table, TableState},
};
#[derive(Debug)]
pub(crate) struct State {
    tasks: HashMap<u64, Rc<RefCell<Task>>>,
    sorted_tasks: Vec<Weak<RefCell<Task>>>,
    sort_by: SortBy,
    table_state: TableState,
    selected_column: usize,
    sort_descending: bool,
    last_updated_at: Option<SystemTime>,
}

#[derive(Debug)]
#[repr(usize)]
enum SortBy {
    Tid = 0,
    Total = 2,
    Busy = 3,
    Idle = 4,
    Polls = 5,
}

#[derive(Debug)]
pub(crate) struct Task {
    id: u64,
    id_hex: String,
    fields: String,
    kind: &'static str,
    stats: Stats,
    completed_for: usize,
}

#[derive(Debug)]
struct Stats {
    polls: u64,
    created_at: SystemTime,
    busy: Duration,
    idle: Option<Duration>,
    total: Option<Duration>,
}

impl State {
    // How many updates to retain completed tasks for
    const RETAIN_COMPLETED_FOR: usize = 6;

    const HEADER: &'static [&'static str] =
        &["TID", "KIND", "TOTAL", "BUSY", "IDLE", "POLLS", "FIELDS"];

    pub(crate) fn len(&self) -> usize {
        self.tasks.len()
    }

    pub(crate) fn last_updated_at(&self) -> Option<SystemTime> {
        self.last_updated_at
    }

    pub(crate) fn update_input(&mut self, event: input::Event) {
        // Clippy likes to remind us that we could use an `if let` here, since
        // the match only has one arm...but this is a `match` because I
        // anticipate adding more cases later...
        #[allow(clippy::single_match)]
        match event {
            input::Event::Key(event) => self.key_input(event),
            _ => {
                // do nothing for now
                // TODO(eliza): mouse input would be cool...
            }
        }
    }

    fn key_input(&mut self, input::KeyEvent { code, .. }: input::KeyEvent) {
        use input::KeyCode::*;
        match code {
            Left => {
                if self.selected_column == 0 {
                    self.selected_column = Self::HEADER.len() - 1;
                } else {
                    self.selected_column -= 1;
                }
            }
            Right => {
                if self.selected_column == Self::HEADER.len() - 1 {
                    self.selected_column = 0;
                } else {
                    self.selected_column += 1;
                }
            }
            Char('i') => self.sort_descending = !self.sort_descending,
            Down => self.scroll_next(),
            Up => self.scroll_prev(),
            _ => {} // do nothing for now...
        }
        if let Ok(sort_by) = SortBy::try_from(self.selected_column) {
            self.sort_by = sort_by;
        }
    }

    pub(crate) fn update_tasks(&mut self, update: proto::tasks::TaskUpdate) {
        if let Some(now) = update.now {
            self.last_updated_at = Some(now.into());
        }
        let mut stats_update = update.stats_update;
        let sorted = &mut self.sorted_tasks;
        let new_tasks = update.new_tasks.into_iter().filter_map(|task| {
            if task.id.is_none() {
                tracing::warn!(?task, "skipping task with no id");
            }
            let kind = match task.kind() {
                proto::tasks::task::Kind::Spawn => "T",
                proto::tasks::task::Kind::Blocking => "B",
            };

            let id = task.id?.id;
            let stats = stats_update.remove(&id)?.into();
            let task = Task {
                id,
                id_hex: format!("{:x}", id),
                fields: task.string_fields,
                kind,
                stats,
                completed_for: 0,
            };
            let task = Rc::new(RefCell::new(task));
            sorted.push(Rc::downgrade(&task));
            Some((id, task))
        });
        self.tasks.extend(new_tasks);

        for (id, stats) in stats_update {
            if let Some(task) = self.tasks.get_mut(&id) {
                task.borrow_mut().stats = stats.into();
            }
        }

        for proto::SpanId { id } in update.completed {
            if let Some(task) = self.tasks.get_mut(&id) {
                let mut task = task.borrow_mut();
                task.kind = "!";
                task.completed_for = 1;
            } else {
                tracing::warn!(?id, "tried to complete a task that didn't exist");
            }
        }
    }

    pub(crate) fn render<B: tui::backend::Backend>(
        &mut self,
        frame: &mut tui::terminal::Frame<B>,
        area: layout::Rect,
    ) {
        let now = if let Some(now) = self.last_updated_at {
            now
        } else {
            // If we have never gotten an update yet, skip...
            return;
        };

        const DUR_LEN: usize = 10;
        // This data is only updated every second, so it doesn't make a ton of
        // sense to have a lot of precision in timestamps (and this makes sure
        // there's room for the unit!)
        const DUR_PRECISION: usize = 4;
        const POLLS_LEN: usize = 5;
        self.sort_by.sort(now, &mut self.sorted_tasks);

        let rows = self.sorted_tasks.iter().filter_map(|task| {
            let task = task.upgrade()?;
            let task = task.borrow();
            let mut row = Row::new(vec![
                Cell::from(task.id_hex.to_string()),
                // TODO(eliza): is there a way to write a `fmt::Debug` impl
                // directly to tui without doing an allocation?
                Cell::from(task.kind),
                Cell::from(format!(
                    "{:>width$.prec$?}",
                    task.total(now),
                    width = DUR_LEN,
                    prec = DUR_PRECISION,
                )),
                Cell::from(format!(
                    "{:>width$.prec$?}",
                    task.busy(),
                    width = DUR_LEN,
                    prec = DUR_PRECISION,
                )),
                Cell::from(format!(
                    "{:>width$.prec$?}",
                    task.idle(now),
                    width = DUR_LEN,
                    prec = DUR_PRECISION,
                )),
                Cell::from(format!("{:>width$}", task.stats.polls, width = POLLS_LEN)),
                Cell::from(task.fields.to_string()),
            ]);
            if task.completed_for > 0 {
                row = row.style(Style::default().add_modifier(style::Modifier::DIM));
            }
            Some(row)
        });

        let block = Block::default().title(vec![
            text::Span::raw("controls: "),
            text::Span::styled(
                "\u{2190}\u{2192}",
                Style::default().add_modifier(style::Modifier::BOLD),
            ),
            text::Span::raw(" = select column (sort), "),
            text::Span::styled(
                "\u{2191}\u{2193}",
                Style::default().add_modifier(style::Modifier::BOLD),
            ),
            text::Span::raw(" = scroll, "),
            text::Span::styled(
                "enter",
                Style::default().add_modifier(style::Modifier::BOLD),
            ),
            text::Span::raw(" = task details, "),
            text::Span::styled("i", Style::default().add_modifier(style::Modifier::BOLD)),
            text::Span::raw(" = invert sort (highest/lowest), "),
            text::Span::styled("q", Style::default().add_modifier(style::Modifier::BOLD)),
            text::Span::raw(" = quit"),
        ]);

        let header = Row::new(Self::HEADER.iter().enumerate().map(|(idx, &value)| {
            let cell = Cell::from(value);
            if idx == self.selected_column {
                cell.style(Style::default().remove_modifier(style::Modifier::REVERSED))
            } else {
                cell
            }
        }))
        .height(1)
        .style(Style::default().add_modifier(style::Modifier::REVERSED));

        let t = if self.sort_descending {
            Table::new(rows)
        } else {
            Table::new(rows.rev())
        };
        let t = t
            .header(header)
            .block(block)
            .widths(&[
                layout::Constraint::Min(20),
                layout::Constraint::Length(4),
                layout::Constraint::Min(DUR_LEN as u16),
                layout::Constraint::Min(DUR_LEN as u16),
                layout::Constraint::Min(DUR_LEN as u16),
                layout::Constraint::Min(POLLS_LEN as u16),
                layout::Constraint::Min(10),
            ])
            .highlight_symbol(">> ")
            .highlight_style(Style::default().add_modifier(style::Modifier::BOLD));

        frame.render_stateful_widget(t, area, &mut self.table_state);
        self.sorted_tasks.retain(|t| t.upgrade().is_some());
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

    fn scroll_next(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                if i >= self.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    pub fn scroll_prev(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    pub(crate) fn selected_task(&self) -> Weak<RefCell<Task>> {
        self.table_state
            .selected()
            .map(|i| {
                let selected = if self.sort_descending {
                    i
                } else {
                    self.sorted_tasks.len() - i - 1
                };
                self.sorted_tasks[selected].clone()
            })
            .unwrap_or_default()
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            tasks: Default::default(),
            sorted_tasks: Default::default(),
            sort_by: Default::default(),
            selected_column: SortBy::default() as usize,
            table_state: Default::default(),
            sort_descending: false,
            last_updated_at: None,
        }
    }
}

impl Task {
    pub(crate) fn id_hex(&self) -> &str {
        &self.id_hex
    }

    pub(crate) fn fields(&self) -> &str {
        &self.fields
    }

    pub(crate) fn total(&self, since: SystemTime) -> Duration {
        self.stats
            .total
            .unwrap_or_else(|| since.duration_since(self.stats.created_at).unwrap())
    }

    pub(crate) fn busy(&self) -> Duration {
        self.stats.busy
    }

    pub(crate) fn idle(&self, since: SystemTime) -> Duration {
        self.stats
            .idle
            .unwrap_or_else(|| self.total(since) - self.busy())
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
            polls: pb.polls,
            created_at: pb.created_at.expect("task span was never created").into(),
        }
    }
}

impl Default for SortBy {
    fn default() -> Self {
        Self::Total
    }
}

impl SortBy {
    fn sort(&self, now: SystemTime, tasks: &mut Vec<Weak<RefCell<Task>>>) {
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
                tasks.sort_unstable_by_key(|task| task.upgrade().map(|t| t.borrow().busy()))
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
