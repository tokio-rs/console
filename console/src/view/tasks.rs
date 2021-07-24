use crate::{
    input,
    tasks::{self, TaskRef},
    view::{self, bold},
};
use std::convert::TryFrom;
use tui::{
    layout,
    style::{self, Style},
    text::{self, Spans},
    widgets::{Block, Cell, Row, Table, TableState},
};
#[derive(Clone, Debug, Default)]
pub(crate) struct List {
    sorted_tasks: Vec<TaskRef>,
    sort_by: tasks::SortBy,
    table_state: TableState,
    selected_column: usize,
    sort_descending: bool,
}

impl List {
    const HEADER: &'static [&'static str] = &[
        "TID", "KIND", "TOTAL", "BUSY", "IDLE", "POLLS", "TARGET", "FIELDS",
    ];

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
        if let Ok(sort_by) = tasks::SortBy::try_from(self.selected_column) {
            self.sort_by = sort_by;
        }
    }

    pub(crate) fn render<B: tui::backend::Backend>(
        &mut self,
        frame: &mut tui::terminal::Frame<B>,
        area: layout::Rect,
        state: &mut tasks::State,
    ) {
        let now = if let Some(now) = state.last_updated_at() {
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
        const MIN_TARGET_LEN: usize = 15;

        self.sorted_tasks.extend(state.take_new_tasks());
        self.sort_by.sort(now, &mut self.sorted_tasks);

        fn dur_cell(dur: std::time::Duration) -> Cell<'static> {
            Cell::from(view::color_time_units(format!(
                "{:>width$.prec$?}",
                dur,
                width = DUR_LEN,
                prec = DUR_PRECISION,
            )))
        }

        let rows = self.sorted_tasks.iter().filter_map(|task| {
            let task = task.upgrade()?;
            let task = task.borrow();

            let mut row = Row::new(vec![
                Cell::from(task.id().to_string()),
                // TODO(eliza): is there a way to write a `fmt::Debug` impl
                // directly to tui without doing an allocation?
                Cell::from(task.kind().to_string()),
                dur_cell(task.total(now)),
                dur_cell(task.busy(now)),
                dur_cell(task.idle(now)),
                Cell::from(format!("{:>width$}", task.total_polls(), width = POLLS_LEN)),
                Cell::from(task.target().to_owned()),
                Cell::from(Spans::from(
                    task.formatted_fields()
                        .iter()
                        .flatten()
                        .cloned()
                        .collect::<Vec<_>>(),
                )),
            ]);
            if task.completed_for() > 0 {
                row = row.style(Style::default().add_modifier(style::Modifier::DIM));
            }
            Some(row)
        });

        let block = Block::default().title(vec![
            text::Span::raw("controls: "),
            bold("\u{2190}\u{2192}"),
            text::Span::raw(" = select column (sort), "),
            bold("\u{2191}\u{2193}"),
            text::Span::raw(" = scroll, "),
            bold("enter"),
            text::Span::raw(" = task details, "),
            bold("i"),
            text::Span::raw(" = invert sort (highest/lowest), "),
            bold("q"),
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
                layout::Constraint::Min(4),
                layout::Constraint::Length(4),
                layout::Constraint::Min(DUR_LEN as u16),
                layout::Constraint::Min(DUR_LEN as u16),
                layout::Constraint::Min(DUR_LEN as u16),
                layout::Constraint::Min(POLLS_LEN as u16),
                layout::Constraint::Min(MIN_TARGET_LEN as u16),
                layout::Constraint::Min(10),
            ])
            .highlight_symbol(">> ")
            .highlight_style(Style::default().add_modifier(style::Modifier::BOLD));

        frame.render_stateful_widget(t, area, &mut self.table_state);
        self.sorted_tasks.retain(|t| t.upgrade().is_some());
    }

    fn scroll_next(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                if i >= self.sorted_tasks.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    fn scroll_prev(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.sorted_tasks.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    pub(crate) fn selected_task(&self) -> TaskRef {
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
