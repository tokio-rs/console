use crate::{
    input,
    state::{tasks::Task, DetailsRef},
    util::Percentage,
    view::{
        self, bold,
        controls::{controls_paragraph, ControlDisplay, Controls, KeyDisplay},
        durations::Durations,
        help::HelpText,
    },
};
use ratatui::{
    layout::{self, Layout},
    text::{Line, Span, Text},
    widgets::{List, ListItem, Paragraph},
};
use std::{
    cell::RefCell,
    cmp,
    rc::Rc,
    time::{Duration, SystemTime},
};

pub(crate) struct TaskView {
    task: Rc<RefCell<Task>>,
    details: DetailsRef,
}

impl TaskView {
    pub(super) fn new(task: Rc<RefCell<Task>>, details: DetailsRef) -> Self {
        TaskView { task, details }
    }

    pub(crate) fn update_input(&mut self, _event: input::Event) {
        // TODO :D
    }
    pub(crate) fn render(
        &mut self,
        styles: &view::Styles,
        frame: &mut ratatui::terminal::Frame,
        area: layout::Rect,
        now: SystemTime,
    ) {
        // Rows with the following info:
        // - Task main attributes
        // - task metadata
        // - metrics
        // - logs?

        let task = &*self.task.borrow();
        let details_ref = self.details.borrow();
        let details = details_ref
            .as_ref()
            .filter(|details| details.span_id() == task.span_id());

        let controls = Controls::new(view_controls(), &area, styles);

        let warnings: Vec<_> = task
            .warnings()
            .iter()
            .map(|linter| {
                ListItem::new(Text::from(Line::from(vec![
                    styles.warning_wide(),
                    // TODO(eliza): it would be nice to handle singular vs plural...
                    Span::from(linter.format(task)),
                ])))
            })
            .collect();

        let stats_area_check = Layout::default()
            .direction(layout::Direction::Horizontal)
            .constraints(
                [
                    layout::Constraint::Percentage(50),
                    layout::Constraint::Percentage(50),
                ]
                .as_ref(),
            )
            .split(area);

        let location_heading = "Location: ";
        let location_max_width = stats_area_check[0].width as usize - 2 - location_heading.len(); // NOTE: -2 for the border
        let max_width_stats_area = area.width - 45;
        let mut location_lines_vector: Vec<String> = task
            .location()
            .to_string()
            .chars()
            .collect::<Vec<char>>()
            .chunks(max_width_stats_area as usize)
            .map(|chunk| chunk.iter().collect())
            .collect();
        let no_of_lines_extra_required_to_accomadate_location = location_lines_vector.len() - 1;
        let (
            controls_area,
            stats_area,
            poll_dur_area,
            scheduled_dur_area,
            fields_area,
            warnings_area,
        ) = if task.location().len() > location_max_width {
            if warnings.is_empty() {
                let chunks = Layout::default()
                    .direction(layout::Direction::Vertical)
                    .constraints(
                        [
                            // controls
                            layout::Constraint::Length(controls.height()),
                            // task stats
                            layout::Constraint::Length(
                                10 + no_of_lines_extra_required_to_accomadate_location as u16,
                            ),
                            // poll duration
                            layout::Constraint::Length(9),
                            // scheduled duration
                            layout::Constraint::Length(9),
                            // fields
                            layout::Constraint::Percentage(60),
                        ]
                        .as_ref(),
                    )
                    .split(area);
                (chunks[0], chunks[1], chunks[2], chunks[3], chunks[4], None)
            } else {
                let chunks = Layout::default()
                    .direction(layout::Direction::Vertical)
                    .constraints(
                        [
                            // controls
                            layout::Constraint::Length(controls.height()),
                            // warnings (add 2 for top and bottom borders)
                            layout::Constraint::Length(warnings.len() as u16 + 2),
                            // task stats
                            layout::Constraint::Length(
                                10 + no_of_lines_extra_required_to_accomadate_location as u16,
                            ),
                            // poll duration
                            layout::Constraint::Length(9),
                            // scheduled duration
                            layout::Constraint::Length(9),
                            // fields
                            layout::Constraint::Percentage(60),
                        ]
                        .as_ref(),
                    )
                    .split(area);

                (
                    chunks[0],
                    chunks[2],
                    chunks[3],
                    chunks[4],
                    chunks[5],
                    Some(chunks[1]),
                )
            }
        } else {
            if warnings.is_empty() {
                let chunks = Layout::default()
                    .direction(layout::Direction::Vertical)
                    .constraints(
                        [
                            // controls
                            layout::Constraint::Length(controls.height()),
                            // task stats
                            layout::Constraint::Length(10),
                            // poll duration
                            layout::Constraint::Length(9),
                            // scheduled duration
                            layout::Constraint::Length(9),
                            // fields
                            layout::Constraint::Percentage(60),
                        ]
                        .as_ref(),
                    )
                    .split(area);
                (chunks[0], chunks[1], chunks[2], chunks[3], chunks[4], None)
            } else {
                let chunks = Layout::default()
                    .direction(layout::Direction::Vertical)
                    .constraints(
                        [
                            // controls
                            layout::Constraint::Length(controls.height()),
                            // warnings (add 2 for top and bottom borders)
                            layout::Constraint::Length(warnings.len() as u16 + 2),
                            // task stats
                            layout::Constraint::Length(10),
                            // poll duration
                            layout::Constraint::Length(9),
                            // scheduled duration
                            layout::Constraint::Length(9),
                            // fields
                            layout::Constraint::Percentage(60),
                        ]
                        .as_ref(),
                    )
                    .split(area);

                (
                    chunks[0],
                    chunks[2],
                    chunks[3],
                    chunks[4],
                    chunks[5],
                    Some(chunks[1]),
                )
            }
        };

        let stats_area = if location_lines_vector.len() != 1 {
            let area_needed_to_render_location = task.location().len() as u16;
            Layout::default()
                .direction(layout::Direction::Horizontal)
                .constraints(
                    [
                        layout::Constraint::Min(area_needed_to_render_location + 15), //Note: 15 is the length of "| Location:     |"
                        layout::Constraint::Min(32),
                    ]
                    .as_ref(),
                )
                .split(stats_area)
        } else {
            Layout::default()
                .direction(layout::Direction::Horizontal)
                .constraints(
                    [
                        layout::Constraint::Percentage(50),
                        layout::Constraint::Percentage(50),
                    ]
                    .as_ref(),
                )
                .split(stats_area)
            // stats_area_check
        };

        // Just preallocate capacity for ID, name, target, total, busy, and idle.
        let mut overview = Vec::with_capacity(8);
        overview.push(Line::from(vec![
            bold("ID: "),
            Span::raw(format!("{} ", task.id_str())),
            task.state().render(styles),
        ]));

        if let Some(name) = task.name() {
            overview.push(Line::from(vec![bold("Name: "), Span::raw(name)]));
        }

        overview.push(Line::from(vec![bold("Target: "), Span::raw(task.target())]));

        let first_line = location_lines_vector[0].clone();
        location_lines_vector.remove(0);
        let location_vector = vec![bold(location_heading), Span::raw(first_line)];
        overview.push(Line::from(location_vector));
        for line in location_lines_vector {
            overview.push(Line::from(Span::raw(format!("    {}", line))));
        }

        let total = task.total(now);

        let dur_percent = |name: &'static str, amt: Duration| -> Line {
            let percent = amt.as_secs_f64().percent_of(total.as_secs_f64());
            Line::from(vec![
                bold(name),
                styles.time_units(amt, view::DUR_LIST_PRECISION, None),
                Span::from(format!(" ({:.2}%)", percent)),
            ])
        };

        overview.push(Line::from(vec![
            bold("Total Time: "),
            styles.time_units(total, view::DUR_LIST_PRECISION, None),
        ]));
        overview.push(dur_percent("Busy: ", task.busy(now)));
        overview.push(dur_percent("Scheduled: ", task.scheduled(now)));
        overview.push(dur_percent("Idle: ", task.idle(now)));

        let mut waker_stats = vec![Line::from(vec![
            bold("Current wakers: "),
            Span::from(format!("{} ", task.waker_count())),
        ])];
        let waker_stats_clones = vec![
            bold("  Clones: "),
            Span::from(format!("{}, ", task.waker_clones())),
        ];

        let waker_stats_drops = vec![
            bold("  Drops: "),
            Span::from(format!("{}", task.waker_drops())),
        ];

        let wakeups = vec![
            bold("Woken: "),
            Span::from(format!("{} times", task.wakes())),
        ];

        let mut last_woken_line = vec![];

        // If the task has been woken, add the time since wake to its stats as well.
        if let Some(since) = task.since_wake(now) {
            last_woken_line.reserve(3);
            last_woken_line.push(bold("Last woken: "));
            last_woken_line.push(styles.time_units(since, view::DUR_LIST_PRECISION, None));
            last_woken_line.push(Span::raw(" ago"));
        }

        waker_stats.push(Line::from(waker_stats_clones));
        waker_stats.push(Line::from(waker_stats_drops));
        waker_stats.push(Line::from(wakeups));
        waker_stats.push(Line::from(last_woken_line));

        if task.self_wakes() > 0 {
            waker_stats.push(Line::from(vec![
                bold("Self Wakes: "),
                Span::from(format!(
                    "{} times ({}%)",
                    task.self_wakes(),
                    task.self_wake_percent()
                )),
            ]));
        }

        let mut fields = Text::default();
        fields.extend(task.formatted_fields().iter().cloned().map(Line::from));

        if let Some(warnings_area) = warnings_area {
            let warnings = List::new(warnings).block(styles.border_block().title("Warnings"));
            frame.render_widget(warnings, warnings_area);
        }

        let task_widget = Paragraph::new(overview).block(styles.border_block().title("Task"));
        let wakers_widget = Paragraph::new(waker_stats).block(styles.border_block().title("Waker"));

        let poll_percentiles_title = "Poll Times Percentiles";
        let scheduled_percentiles_title = "Sched Times Percentiles";
        let percentiles_width = cmp::max(
            poll_percentiles_title.len(),
            scheduled_percentiles_title.len(),
        ) as u16
            + 2_u16; // extra 2 characters for the border
        let poll_durations_widget = Durations::new(styles)
            .histogram(details.and_then(|d| d.poll_times_histogram()))
            .percentiles_title(poll_percentiles_title)
            .histogram_title("Poll Times Histogram")
            .percentiles_width(percentiles_width);
        let scheduled_durations_widget = Durations::new(styles)
            .histogram(details.and_then(|d| d.scheduled_times_histogram()))
            .percentiles_title(scheduled_percentiles_title)
            .histogram_title("Scheduled Times Histogram")
            .percentiles_width(percentiles_width);

        let fields_widget = Paragraph::new(fields).block(styles.border_block().title("Fields"));

        frame.render_widget(controls.into_widget(), controls_area);
        frame.render_widget(task_widget, stats_area[0]);
        frame.render_widget(wakers_widget, stats_area[1]);
        frame.render_widget(poll_durations_widget, poll_dur_area);
        frame.render_widget(scheduled_durations_widget, scheduled_dur_area);
        frame.render_widget(fields_widget, fields_area);
    }
}

impl HelpText for TaskView {
    fn render_help_content(&self, styles: &view::Styles) -> Paragraph<'static> {
        controls_paragraph(view_controls(), styles)
    }
}

const fn view_controls() -> &'static [ControlDisplay] {
    &[ControlDisplay {
        action: "return to task list",
        keys: &[KeyDisplay {
            base: "esc",
            utf8: Some("\u{238B} esc"),
        }],
    }]
}
