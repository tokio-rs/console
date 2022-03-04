use crate::{
    state::{
        tasks::{SortBy, Task, TaskState},
        State,
    },
    view::{
        self, bold,
        table::{self, TableList, TableListState},
        DUR_LEN, DUR_PRECISION,
    },
};
use tui::{
    layout,
    style::{self, Color, Style},
    text::{Span, Spans, Text},
    widgets::{self, Cell, ListItem, Row, Table},
};

#[derive(Debug, Default)]
pub(crate) struct TasksTable {}

impl TableList for TasksTable {
    type Row = Task;
    type Sort = SortBy;
    type Context = ();

    const HEADER: &'static [&'static str] = &[
        "Warn", "ID", "State", "Name", "Total", "Busy", "Idle", "Polls", "Target", "Location",
        "Fields",
    ];

    fn render<B: tui::backend::Backend>(
        table_list_state: &mut TableListState<Self>,
        styles: &view::Styles,
        frame: &mut tui::terminal::Frame<B>,
        area: layout::Rect,
        state: &mut State,
        _: Self::Context,
    ) {
        let mut state_len: u16 = Self::HEADER[2].len() as u16;
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
            Cell::from(styles.time_units(format!(
                "{:>width$.prec$?}",
                dur,
                width = DUR_LEN,
                prec = DUR_PRECISION,
            )))
        };

        // Start out wide enough to display the column headers...
        let mut warn_width = view::Width::new(Self::HEADER[0].len() as u16);
        let mut id_width = view::Width::new(Self::HEADER[1].len() as u16);
        let mut name_width = view::Width::new(Self::HEADER[3].len() as u16);
        let mut polls_width = view::Width::new(Self::HEADER[7].len() as u16);
        let mut target_width = view::Width::new(Self::HEADER[8].len() as u16);
        let mut location_width = view::Width::new(Self::HEADER[9].len() as u16);

        let mut num_idle = 0;
        let mut num_running = 0;

        match table_list_state.sort_by {
            SortBy::Warns => warn_width.update_len(warn_width.len() + 1),
            SortBy::Tid => id_width.update_len(id_width.len() + 1),
            SortBy::State => state_len += 1,
            SortBy::Name => name_width.update_len(name_width.len() + 1),
            SortBy::Polls => polls_width.update_len(polls_width.len() + 1),
            SortBy::Target => target_width.update_len(target_width.len() + 1),
            SortBy::Location => location_width.update_len(location_width.len() + 1),
            SortBy::Total | SortBy::Busy | SortBy::Idle => (),
        };

        let rows = {
            let id_width = &mut id_width;
            let target_width = &mut target_width;
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
                        Cell::from(Spans::from(vec![
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
                            task.id(),
                            width = id_width.chars() as usize
                        ))),
                        Cell::from(task.state().render(styles)),
                        Cell::from(name_width.update_str(task.name().unwrap_or("").to_string())),
                        dur_cell(task.total(now)),
                        dur_cell(task.busy(now)),
                        dur_cell(task.idle(now)),
                        Cell::from(polls_width.update_str(task.total_polls().to_string())),
                        Cell::from(target_width.update_str(task.target()).to_owned()),
                        Cell::from(location_width.update_str(task.location().to_owned())),
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
            if idx == table_list_state.selected_column {
                let suffix = if table_list_state.sort_descending {
                    "▵"
                } else {
                    "▿"
                };
                Cell::from(format!("{}{}", value, suffix)).style(selected_style)
            } else {
                Cell::from(value)
            }
        }))
        .height(1)
        .style(header_style);

        let table = if table_list_state.sort_descending {
            Table::new(rows)
        } else {
            Table::new(rows.rev())
        };

        let block = styles.border_block().title(vec![
            bold(format!("Tasks ({}) ", table_list_state.len())),
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
        let warnings = state
            .tasks_state()
            .warnings()
            .map(|warning| {
                ListItem::new(Text::from(Spans::from(vec![
                    styles.warning_wide(),
                    // TODO(eliza): it would be nice to handle singular vs plural...
                    Span::from(format!("{} {}", warning.count(), warning.summary())),
                ])))
            })
            .collect::<Vec<_>>();

        let layout = layout::Layout::default()
            .direction(layout::Direction::Vertical)
            .margin(0);

        let controls = table::Controls::for_area(&area, styles);

        let (controls_area, tasks_area, warnings_area) = if warnings.is_empty() {
            let chunks = layout
                .constraints(
                    [
                        layout::Constraint::Length(controls.height),
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
                        layout::Constraint::Length(controls.height),
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
            polls_width.constraint(),
            target_width.constraint(),
            location_width.constraint(),
            fields_width,
        ];

        let table = table
            .header(header)
            .block(block)
            .widths(widths)
            .highlight_symbol(view::TABLE_HIGHLIGHT_SYMBOL)
            .highlight_style(Style::default().add_modifier(style::Modifier::BOLD));

        frame.render_stateful_widget(table, tasks_area, &mut table_list_state.table_state);
        frame.render_widget(controls.paragraph, controls_area);

        if let Some(area) = warnings_area {
            let block = styles
                .border_block()
                .title(Spans::from(vec![bold("Warnings")]));
            frame.render_widget(widgets::List::new(warnings).block(block), area);
        }

        table_list_state
            .sorted_items
            .retain(|t| t.upgrade().is_some());
    }
}
