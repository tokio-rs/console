use crate::{
    input,
    tasks::{self, TaskRef, TaskState},
    view::{self, bold},
};
use std::convert::TryFrom;
use tui::{
    layout,
    style::{self, Color, Style},
    text::{self, Span, Spans},
    widgets::{Cell, Paragraph, Row, Table, TableState},
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
        "ID", "State", "Name", "Total", "Busy", "Idle", "Polls", "Target", "Fields",
    ];

    pub(crate) fn len(&self) -> usize {
        self.sorted_tasks.len()
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
        if let Ok(sort_by) = tasks::SortBy::try_from(self.selected_column) {
            self.sort_by = sort_by;
        }
    }

    pub(crate) fn render<B: tui::backend::Backend>(
        &mut self,
        styles: &view::Styles,
        frame: &mut tui::terminal::Frame<B>,
        area: layout::Rect,
        state: &mut tasks::State,
    ) {
        let chunks = layout::Layout::default()
            .direction(layout::Direction::Vertical)
            .margin(0)
            .constraints(
                [
                    layout::Constraint::Length(1),
                    layout::Constraint::Min(area.height - 1),
                ]
                .as_ref(),
            )
            .split(area);
        let controls_area = chunks[0];
        let tasks_area = chunks[1];

        let now = if let Some(now) = state.last_updated_at() {
            now
        } else {
            // If we have never gotten an update yet, skip...
            return;
        };

        const STATE_LEN: u16 = List::HEADER[1].len() as u16;
        const DUR_LEN: usize = 10;
        // This data is only updated every second, so it doesn't make a ton of
        // sense to have a lot of precision in timestamps (and this makes sure
        // there's room for the unit!)
        const DUR_PRECISION: usize = 4;
        const POLLS_LEN: usize = 5;

        self.sorted_tasks.extend(state.take_new_tasks());
        self.sort_by.sort(now, &mut self.sorted_tasks);

        let dur_cell = |dur: std::time::Duration| -> Cell<'static> {
            Cell::from(styles.time_units(format!(
                "{:>width$.prec$?}",
                dur,
                width = DUR_LEN,
                prec = DUR_PRECISION,
            )))
        };

        // Start out wide enough to display the column headers...
        let mut id_width = view::Width::new(Self::HEADER[0].len() as u16);
        let mut name_width = view::Width::new(Self::HEADER[2].len() as u16);
        let mut target_width = view::Width::new(Self::HEADER[7].len() as u16);
        let mut num_idle = 0;
        let mut num_running = 0;
        let rows = {
            let id_width = &mut id_width;
            let target_width = &mut target_width;
            let name_width = &mut name_width;
            let num_running = &mut num_running;
            let num_idle = &mut num_idle;
            self.sorted_tasks.iter().filter_map(move |task| {
                let task = task.upgrade()?;
                let task = task.borrow();
                let state = task.state();

                // Count task states
                match state {
                    TaskState::Running => *num_running += 1,
                    TaskState::Idle => *num_idle += 1,
                    _ => {}
                };

                let mut row = Row::new(vec![
                    Cell::from(id_width.update_str(format!(
                        "{:>width$}",
                        task.id(),
                        width = id_width.chars() as usize
                    ))),
                    Cell::from(task.state().render(styles)),
                    Cell::from(name_width.update_str(task.name().to_string())),
                    dur_cell(task.total(now)),
                    dur_cell(task.busy(now)),
                    dur_cell(task.idle(now)),
                    Cell::from(format!("{:>width$}", task.total_polls(), width = POLLS_LEN)),
                    Cell::from(target_width.update_str(task.target()).to_owned()),
                    Cell::from(Spans::from(
                        task.formatted_fields()
                            .iter()
                            .flatten()
                            .cloned()
                            .collect::<Vec<_>>(),
                    )),
                ]);
                if state == TaskState::Completed {
                    row = row.style(styles.terminated());
                }
                Some(row)
            })
        };

        let (selected_style, header_style) = if let Some(cyan) = styles.color(Color::Cyan) {
            (Style::default().fg(cyan), Style::default())
        } else {
            (
                Style::default().remove_modifier(style::Modifier::REVERSED),
                Style::default().add_modifier(style::Modifier::REVERSED),
            )
        };
        let header_style = header_style.add_modifier(style::Modifier::BOLD);

        let header = Row::new(Self::HEADER.iter().enumerate().map(|(idx, &value)| {
            let cell = Cell::from(value);
            if idx == self.selected_column {
                cell.style(selected_style)
            } else {
                cell
            }
        }))
        .height(1)
        .style(header_style);

        let table = if self.sort_descending {
            Table::new(rows)
        } else {
            Table::new(rows.rev())
        };

        let block = styles.border_block().title(vec![
            bold(format!("Tasks ({}) ", self.len())),
            TaskState::Running.render(styles),
            Span::from(format!(" Running ({}) ", num_running)),
            TaskState::Idle.render(styles),
            Span::from(format!(" Idle ({})", num_idle)),
        ]);

        /* TODO: use this to adjust the max size of name and target columns...
        // How many characters wide are the fixed-length non-field columns?
        let fixed_col_width = id_width.chars()
            + STATE_LEN
            + name_width.chars()
            + DUR_LEN as u16
            + DUR_LEN as u16
            + DUR_LEN as u16
            + POLLS_LEN as u16
            + target_width.chars();
        */

        // Fill all remaining characters in the frame with the task's fields.
        //
        // Ideally we'd use Min(0), and it would fill the rest of the space. But that is broken
        // in tui 0.16. We can use Percentage to fill the space for now.
        //
        // See https://github.com/fdehau/tui-rs/issues/525
        let fields_width = layout::Constraint::Percentage(100);
        let widths = &[
            id_width.constraint(),
            layout::Constraint::Length(STATE_LEN),
            name_width.constraint(),
            layout::Constraint::Length(DUR_LEN as u16),
            layout::Constraint::Length(DUR_LEN as u16),
            layout::Constraint::Length(DUR_LEN as u16),
            layout::Constraint::Length(POLLS_LEN as u16),
            target_width.constraint(),
            fields_width,
        ];

        let table = table
            .header(header)
            .block(block)
            .widths(widths)
            .highlight_symbol(">> ")
            .highlight_style(Style::default().add_modifier(style::Modifier::BOLD));

        frame.render_stateful_widget(table, tasks_area, &mut self.table_state);

        let controls = tui::text::Text::from(Spans::from(vec![
            Span::raw("controls: "),
            bold(styles.if_utf8("\u{2190}\u{2192}", "left, right")),
            text::Span::raw(" = select column (sort), "),
            bold(styles.if_utf8("\u{2191}\u{2193}", "up, down")),
            text::Span::raw(" = scroll, "),
            bold(styles.if_utf8("\u{21B5}", "enter")),
            text::Span::raw(" = task details, "),
            bold("i"),
            text::Span::raw(" = invert sort (highest/lowest), "),
            bold("q"),
            text::Span::raw(" = quit"),
        ]));

        frame.render_widget(Paragraph::new(controls), controls_area);

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
