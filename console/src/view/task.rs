use crate::{
    input,
    tasks::{DetailsRef, Task},
    util::Percentage,
    view::{self, bold, mini_histogram::MiniHistogram, percentiles::Percentiles},
};
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, SystemTime},
};
use tui::{
    layout::{self, Layout},
    text::{Span, Spans, Text},
    widgets::{Block, List, ListItem, Paragraph},
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

    pub(crate) fn render<B: tui::backend::Backend>(
        &mut self,
        styles: &view::Styles,
        frame: &mut tui::terminal::Frame<B>,
        area: layout::Rect,
        now: SystemTime,
    ) {
        // Rows with the following info:
        // - Task main attributes
        // - task metadata
        // - metrics
        // - logs?

        let task = &*self.task.borrow();

        let warnings: Vec<_> = task
            .warnings()
            .iter()
            .map(|linter| {
                ListItem::new(Text::from(Spans::from(vec![
                    styles.warning_wide(),
                    // TODO(eliza): it would be nice to handle singular vs plural...
                    Span::from(linter.format(task)),
                ])))
            })
            .collect();

        let (controls_area, stats_area, poll_dur_area, fields_area, warnings_area) =
            if warnings.is_empty() {
                let chunks = Layout::default()
                    .direction(layout::Direction::Vertical)
                    .constraints(
                        [
                            // controls
                            layout::Constraint::Length(1),
                            // task stats
                            layout::Constraint::Length(8),
                            // poll duration
                            layout::Constraint::Length(9),
                            // fields
                            layout::Constraint::Percentage(60),
                        ]
                        .as_ref(),
                    )
                    .split(area);
                (chunks[0], chunks[1], chunks[2], chunks[3], None)
            } else {
                let chunks = Layout::default()
                    .direction(layout::Direction::Vertical)
                    .constraints(
                        [
                            // controls
                            layout::Constraint::Length(1),
                            // warnings (add 2 for top and bottom borders)
                            layout::Constraint::Length(warnings.len() as u16 + 2),
                            // task stats
                            layout::Constraint::Length(8),
                            // poll duration
                            layout::Constraint::Length(9),
                            // fields
                            layout::Constraint::Percentage(60),
                        ]
                        .as_ref(),
                    )
                    .split(area);

                (chunks[0], chunks[2], chunks[3], chunks[4], Some(chunks[1]))
            };

        let stats_area = Layout::default()
            .direction(layout::Direction::Horizontal)
            .constraints(
                [
                    layout::Constraint::Percentage(50),
                    layout::Constraint::Percentage(50),
                ]
                .as_ref(),
            )
            .split(stats_area);

        let controls = Spans::from(vec![
            Span::raw("controls: "),
            bold(styles.if_utf8("\u{238B} esc", "esc")),
            Span::raw(" = return to task list, "),
            bold("q"),
            Span::raw(" = quit"),
        ]);

        // Just preallocate capacity for ID, name, target, total, busy, and idle.
        let mut metrics = Vec::with_capacity(6);
        metrics.push(Spans::from(vec![
            bold("ID: "),
            Span::raw(format!("{} ", task.id())),
            task.state().render(styles),
        ]));

        if let Some(name) = task.name() {
            metrics.push(Spans::from(vec![bold("Name: "), Span::raw(name)]));
        }

        metrics.push(Spans::from(vec![
            bold("Target: "),
            Span::raw(task.target()),
        ]));

        let total = task.total(now);

        let dur_percent = |name: &'static str, amt: Duration| -> Spans {
            let percent = amt.as_secs_f64().percent_of(total.as_secs_f64());
            Spans::from(vec![
                bold(name),
                styles.dur(amt),
                Span::from(format!(" ({:.2}%)", percent)),
            ])
        };

        metrics.push(Spans::from(vec![bold("Total Time: "), styles.dur(total)]));
        metrics.push(dur_percent("Busy: ", task.busy(now)));
        metrics.push(dur_percent("Idle: ", task.idle(now)));

        let mut waker_stats = vec![Spans::from(vec![
            bold("Current wakers: "),
            Span::from(format!("{} (", task.waker_count())),
            bold("clones: "),
            Span::from(format!("{}, ", task.waker_clones())),
            bold("drops: "),
            Span::from(format!("{})", task.waker_drops())),
        ])];

        let mut wakeups = vec![
            bold("Woken: "),
            Span::from(format!("{} times", task.wakes())),
        ];

        // If the task has been woken, add the time since wake to its stats as well.
        if let Some(since) = task.since_wake(now) {
            wakeups.reserve(3);
            wakeups.push(Span::raw(", "));
            wakeups.push(bold("last woken:"));
            wakeups.push(Span::from(format!(" {:?} ago", since)));
        }

        waker_stats.push(Spans::from(wakeups));

        if task.self_wakes() > 0 {
            waker_stats.push(Spans::from(vec![
                bold("Self Wakes: "),
                Span::from(format!(
                    "{} times ({}%)",
                    task.self_wakes(),
                    task.self_wake_percent()
                )),
            ]));
        }

        let mut fields = Text::default();
        fields.extend(task.formatted_fields().iter().cloned().map(Spans::from));

        if let Some(warnings_area) = warnings_area {
            let warnings = List::new(warnings).block(styles.border_block().title("Warnings"));
            frame.render_widget(warnings, warnings_area);
        }

        let task_widget = Paragraph::new(metrics).block(styles.border_block().title("Task"));
        let wakers_widget = Paragraph::new(waker_stats).block(styles.border_block().title("Waker"));
        let fields_widget = Paragraph::new(fields).block(styles.border_block().title("Fields"));

        frame.render_widget(Block::default().title(controls), controls_area);
        frame.render_widget(task_widget, stats_area[0]);
        frame.render_widget(wakers_widget, stats_area[1]);
        frame.render_widget(fields_widget, fields_area);
        self.render_poll_dur(styles, frame, poll_dur_area);
    }

    fn render_poll_dur<B: tui::backend::Backend>(
        &self,
        styles: &view::Styles,
        frame: &mut tui::terminal::Frame<B>,
        area: layout::Rect,
    ) {
        let task = &*self.task.borrow();
        let details_ref = self.details.borrow();
        let details = details_ref
            .as_ref()
            .filter(|details| details.task_id() == task.id());
        let histogram = details
            .map(|d| d.poll_times_histogram())
            .unwrap_or_default();

        // Only render the histogram if UTF-8 is enabled,
        // because sparkline requires UTF-8 characters
        let percentiles_area = if styles.utf8 {
            let areas = Layout::default()
                .direction(layout::Direction::Horizontal)
                .constraints(
                    [
                        // 24 chars is long enough for the title "Poll Times Percentiles"
                        layout::Constraint::Length(24),
                        layout::Constraint::Min(50),
                    ]
                    .as_ref(),
                )
                .split(area);
            let (percentiles_area, histogram_area) = (areas[0], areas[1]);

            if let Some(histogram) = histogram {
                let histogram_widget = MiniHistogram::default()
                    .block(styles.border_block().title("Poll Times Histogram"))
                    .histogram(histogram)
                    .duration_precision(2);
                frame.render_widget(histogram_widget, histogram_area);
            } else {
                let histogram_widget = styles.border_block().title("Poll Times Histogram");
                frame.render_widget(histogram_widget, histogram_area);
            }

            percentiles_area
        } else {
            area
        };

        if let Some(histogram) = histogram {
            let percentiles_widget = Percentiles::default()
                .block(styles.border_block().title("Poll Times Percentiles"))
                .styles(styles)
                .histogram(histogram);
            frame.render_widget(percentiles_widget, percentiles_area);
        } else {
            let percentiles_widget = styles.border_block().title("Poll Times Percentiles");
            frame.render_widget(percentiles_widget, percentiles_area);
        }
    }
}
