use crate::{
    state::{
        tasks::{SortBy, Task, TaskState},
        State,
    },
    view::{
        self, bold,
        controls::Controls,
        table::{view_controls, TableList, TableListState},
        DUR_LEN, DUR_TABLE_PRECISION,
    },
};
use ratatui::{
    layout,
    style::{self, Color, Style},
    text::{Line, Span, Text},
    widgets::{self, Cell, ListItem, Row, Table},
};

#[derive(Debug, Default)]
pub(crate) struct TasksTable {}

impl TableList<12> for TasksTable {
    type Row = Task;
    type Sort = SortBy;
    type Context = ();

    const HEADER: &'static [&'static str; 12] = &[
        "Warn", "ID", "State", "Name", "Total", "Busy", "Sched", "Idle", "Polls", "Kind",
        "Location", "Fields",
    ];

    const WIDTHS: &'static [usize; 12] = &[
        Self::HEADER[0].len() + 1,
        Self::HEADER[1].len() + 1,
        Self::HEADER[2].len() + 1,
        Self::HEADER[3].len() + 1,
        Self::HEADER[4].len() + 1,
        Self::HEADER[5].len() + 1,
        Self::HEADER[6].len() + 1,
        Self::HEADER[7].len() + 1,
        Self::HEADER[8].len() + 1,
        Self::HEADER[9].len() + 1,
        Self::HEADER[10].len() + 1,
        Self::HEADER[11].len() + 1,
    ];

    fn render(
        table_list_state: &mut TableListState<Self, 12>,
        styles: &view::Styles,
        frame: &mut ratatui::Frame,
        area: layout::Rect,
        state: &mut State,
        _: Self::Context,
    ) {
        let state_len: u16 = Self::WIDTHS[2] as u16;
        let now = if let Some(now) = state.last_updated_at() {
            now
        } else {
            // If we have never gotten an update yet, skip...
            return;
        };

        table_list_state
            .sorted_items
            .extend(state.tasks_state_mut().take_new_tasks());

        table_list_state
            .sort_by
            .sort(now, &mut table_list_state.sorted_items);

        let dur_cell = |dur: std::time::Duration| -> Cell<'static> {
            Cell::from(styles.time_units(dur, DUR_TABLE_PRECISION, Some(DUR_LEN)))
        };

        // Start out wide enough to display the column headers...
        let mut warn_width = view::Width::new(Self::WIDTHS[0] as u16);
        let mut id_width = view::Width::new(Self::WIDTHS[1] as u16);
        let mut name_width = view::Width::new(Self::WIDTHS[3] as u16);
        let mut polls_width = view::Width::new(Self::WIDTHS[7] as u16);
        let mut kind_width = view::Width::new(Self::WIDTHS[8] as u16);
        let mut location_width = view::Width::new(Self::WIDTHS[9] as u16);

        let mut num_idle = 0;
        let mut num_running = 0;

        let rows = {
            let id_width = &mut id_width;
            let kind_width = &mut kind_width;
            let location_width = &mut location_width;
            let name_width = &mut name_width;
            let polls_width = &mut polls_width;
            let warn_width = &mut warn_width;
            let num_running = &mut num_running;
            let num_idle = &mut num_idle;

            table_list_state
                .sorted_items
                .iter()
                .filter_map(move |task| {
                    let task = task.upgrade()?;
                    let task = task.borrow();
                    let state = task.state();

                    // Count task states
                    match state {
                        TaskState::Running => *num_running += 1,
                        TaskState::Idle => *num_idle += 1,
                        _ => {}
                    };
                    let n_warnings = task.warnings().len();
                    let warnings = if n_warnings > 0 {
                        let n_warnings = n_warnings.to_string();
                        warn_width.update_len(n_warnings.len() + 2); // add 2 for the warning icon + whitespace
                        Cell::from(Line::from(vec![
                            styles.warning_narrow(),
                            Span::from(n_warnings),
                        ]))
                    } else {
                        Cell::from("")
                    };

                    let mut row = Row::new(vec![
                        warnings,
                        Cell::from(id_width.update_str(format!(
                            "{:>width$}",
                            task.id_str(),
                            width = id_width.chars() as usize
                        ))),
                        Cell::from(task.state().render(styles)),
                        Cell::from(name_width.update_str(task.name().unwrap_or("")).to_string()),
                        dur_cell(task.total(now)),
                        dur_cell(task.busy(now)),
                        dur_cell(task.scheduled(now)),
                        dur_cell(task.idle(now)),
                        Cell::from(polls_width.update_str(task.total_polls().to_string())),
                        Cell::from(kind_width.update_str(task.kind()).to_owned()),
                        Cell::from(location_width.update_str(task.location()).to_owned()),
                        Cell::from(Line::from(
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

        let header_style = if styles.color(Color::Cyan).is_some() {
            Style::default()
        } else {
            Style::default().add_modifier(style::Modifier::REVERSED)
        };
        let header_style = header_style.add_modifier(style::Modifier::BOLD);

        let header = Row::new(Self::HEADER.iter().enumerate().map(|(idx, &value)| {
            if idx == table_list_state.selected_column {
                if table_list_state.sort_descending {
                    Cell::from(styles.ascending(value))
                } else {
                    Cell::from(styles.descending(value))
                }
            } else {
                Cell::from(value)
            }
        }))
        .height(1)
        .style(header_style);

        let table = if table_list_state.sort_descending {
            Table::default().rows(rows)
        } else {
            Table::default().rows(rows.rev())
        };

        let block = styles.border_block().title(vec![
            bold(format!("Tasks ({}) ", table_list_state.len())),
            TaskState::Running.render(styles),
            Span::from(format!(" Running ({}) ", num_running)),
            TaskState::Idle.render(styles),
            Span::from(format!(" Idle ({})", num_idle)),
        ]);

        /* TODO: use this to adjust the max size of name and kind columns...
        // How many characters wide are the fixed-length non-field columns?
        let fixed_col_width = id_width.chars()
            + STATE_LEN
            + name_width.chars()
            + DUR_LEN as u16
            + DUR_LEN as u16
            + DUR_LEN as u16
            + POLLS_LEN as u16
            + kind_width.chars();
        */
        let warnings = state
            .tasks_state()
            .warnings()
            .map(|warning| {
                ListItem::new(Text::from(Line::from(vec![
                    styles.warning_wide(),
                    // TODO(eliza): it would be nice to handle singular vs plural...
                    Span::from(format!("{} {}", warning.count(), warning.summary())),
                ])))
            })
            .collect::<Vec<_>>();

        let layout = layout::Layout::default()
            .direction(layout::Direction::Vertical)
            .margin(0);

        let controls = Controls::new(view_controls(), &area, styles);

        let (controls_area, tasks_area, warnings_area) = if warnings.is_empty() {
            let chunks = layout
                .constraints(
                    [
                        layout::Constraint::Length(controls.height()),
                        layout::Constraint::Max(area.height),
                    ]
                    .as_ref(),
                )
                .split(area);
            (chunks[0], chunks[1], None)
        } else {
            let warnings_height = warnings.len() as u16 + 2;
            let chunks = layout
                .constraints(
                    [
                        layout::Constraint::Length(controls.height()),
                        layout::Constraint::Length(warnings_height),
                        layout::Constraint::Max(area.height),
                    ]
                    .as_ref(),
                )
                .split(area);
            (chunks[0], chunks[2], Some(chunks[1]))
        };
        // Fill all remaining characters in the frame with the task's fields.
        //
        // Ideally we'd use Min(0), and it would fill the rest of the space. But that is broken
        // in tui 0.16. We can use Percentage to fill the space for now.
        //
        // See https://github.com/fdehau/tui-rs/issues/525
        let fields_width = layout::Constraint::Percentage(100);
        let widths = &[
            warn_width.constraint(),
            id_width.constraint(),
            layout::Constraint::Length(state_len),
            name_width.constraint(),
            layout::Constraint::Length(DUR_LEN as u16),
            layout::Constraint::Length(DUR_LEN as u16),
            layout::Constraint::Length(DUR_LEN as u16),
            layout::Constraint::Length(DUR_LEN as u16),
            polls_width.constraint(),
            kind_width.constraint(),
            location_width.constraint(),
            fields_width,
        ];

        let table = table
            .header(header)
            .block(block)
            .widths(widths)
            .highlight_symbol(view::TABLE_HIGHLIGHT_SYMBOL)
            .row_highlight_style(Style::default().add_modifier(style::Modifier::BOLD));

        frame.render_stateful_widget(table, tasks_area, &mut table_list_state.table_state);
        frame.render_widget(controls.into_widget(), controls_area);

        if let Some(area) = warnings_area {
            let block = styles
                .border_block()
                .title(Line::from(vec![bold("Warnings")]));
            frame.render_widget(widgets::List::new(warnings).block(block), area);
        }

        table_list_state
            .sorted_items
            .retain(|t| t.upgrade().is_some());
    }
}
