use crate::{
    input,
    tasks::{Details, DetailsRef, Task},
    view::{
        self, bold,
        mini_histogram::{HistogramMetadata, MiniHistogram},
    },
};
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, SystemTime},
};
use tui::{
    layout::{self, Layout},
    text::{Span, Spans, Text},
    widgets::{Block, Paragraph},
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
        let details_ref = self.details.borrow();
        let details = details_ref
            .as_ref()
            .filter(|details| details.task_id() == task.id());

        let chunks = Layout::default()
            .direction(layout::Direction::Vertical)
            .constraints(
                [
                    layout::Constraint::Length(1),
                    layout::Constraint::Length(7),
                    layout::Constraint::Length(9),
                    layout::Constraint::Percentage(60),
                ]
                .as_ref(),
            )
            .split(area);

        let controls_area = chunks[0];
        let stats_area = Layout::default()
            .direction(layout::Direction::Horizontal)
            .constraints(
                [
                    layout::Constraint::Percentage(50),
                    layout::Constraint::Percentage(50),
                ]
                .as_ref(),
            )
            .split(chunks[1]);

        // Only split the histogram area in half if we're also drawing a
        // sparkline (which requires UTF-8 characters).
        let histogram_area = if styles.utf8 {
            Layout::default()
                .direction(layout::Direction::Horizontal)
                .constraints(
                    [
                        // 24 chars is long enough for the title "Poll Times Percentiles"
                        layout::Constraint::Length(24),
                        layout::Constraint::Min(50),
                    ]
                    .as_ref(),
                )
                .split(chunks[2])
        } else {
            vec![chunks[2]]
        };

        let percentiles_area = histogram_area[0];

        let fields_area = chunks[3];

        let controls = Spans::from(vec![
            Span::raw("controls: "),
            bold(styles.if_utf8("\u{238B} esc", "esc")),
            Span::raw(" = return to task list, "),
            bold("q"),
            Span::raw(" = quit"),
        ]);

        let attrs = Spans::from(vec![
            bold("ID: "),
            Span::raw(format!("{} ", task.id())),
            task.state().render(styles),
        ]);
        let target = Spans::from(vec![bold("Target: "), Span::raw(task.target())]);
        let total = Spans::from(vec![bold("Total Time: "), dur(styles, task.total(now))]);
        let busy = Spans::from(vec![bold("Busy: "), dur(styles, task.busy(now))]);
        let idle = Spans::from(vec![bold("Idle: "), dur(styles, task.idle(now))]);

        let metrics = vec![attrs, target, total, busy, idle];

        let wakers = Spans::from(vec![
            bold("Current wakers: "),
            Span::from(format!("{} (", task.waker_count())),
            bold("clones: "),
            Span::from(format!("{}, ", task.waker_clones())),
            bold("drops: "),
            Span::from(format!("{})", task.waker_drops())),
        ]);

        let mut wakeups = vec![
            bold("Woken: "),
            Span::from(format!("{} times", task.wakes())),
        ];

        // If the task has been woken, add the time since wake to its stats as well.
        if let Some(since) = task.since_wake(now) {
            wakeups.push(Span::raw(", "));
            wakeups.push(bold("last woken:"));
            wakeups.push(Span::from(format!(" {:?} ago", since)));
        }

        let wakeups = Spans::from(wakeups);

        let mut fields = Text::default();
        fields.extend(task.formatted_fields().iter().cloned().map(Spans::from));

        // If UTF-8 is disabled we can't draw the histogram sparklne.
        if styles.utf8 {
            let sparkline_area = histogram_area[1];

            // Bit of a deadlock: We cannot know the highest bucket value without determining the number of buckets,
            // and we cannot determine the number of buckets without knowing the width of the chart area which depends on
            // the number of digits in the highest bucket value.
            // So just assume here the number of digits in the highest bucket value is 3.
            // If we overshoot, there will be empty columns/buckets at the right end of the chart.
            // If we undershoot, the rightmost 1-2 columns/buckets will be hidden.
            // We could get the max bucket value from the previous render though...
            let (chart_data, metadata) = details
                .map(|d| d.make_chart_data(sparkline_area.width - 3))
                .unwrap_or_default();

            let histogram_sparkline = MiniHistogram::default()
                .block(styles.border_block().title("Poll Times Histogram"))
                .data(&chart_data)
                .metadata(metadata)
                .duration_precision(2);

            frame.render_widget(histogram_sparkline, sparkline_area);
        }

        let task_widget = Paragraph::new(metrics).block(styles.border_block().title("Task"));
        let wakers_widget =
            Paragraph::new(vec![wakers, wakeups]).block(styles.border_block().title("Waker"));
        let fields_widget = Paragraph::new(fields).block(styles.border_block().title("Fields"));
        let percentiles_widget = Paragraph::new(
            details
                .map(|details| details.make_percentiles_widget(styles))
                .unwrap_or_default(),
        )
        .block(styles.border_block().title("Poll Times Percentiles"));

        frame.render_widget(Block::default().title(controls), controls_area);
        frame.render_widget(task_widget, stats_area[0]);
        frame.render_widget(wakers_widget, stats_area[1]);
        frame.render_widget(fields_widget, fields_area);
        frame.render_widget(percentiles_widget, percentiles_area);
    }
}

impl Details {
    /// From the histogram, build a visual representation by trying to make as
    // many buckets as the width of the render area.
    fn make_chart_data(&self, width: u16) -> (Vec<u64>, HistogramMetadata) {
        self.poll_times_histogram()
            .map(|histogram| {
                let step_size =
                    ((histogram.max() - histogram.min()) as f64 / width as f64).ceil() as u64 + 1;
                // `iter_linear` panics if step_size is 0
                let data = if step_size > 0 {
                    let mut found_first_nonzero = false;
                    let data: Vec<u64> = histogram
                        .iter_linear(step_size)
                        .filter_map(|value| {
                            let count = value.count_since_last_iteration();
                            // Remove the 0s from the leading side of the buckets.
                            // Because HdrHistogram can return empty buckets depending
                            // on its internal state, as it approximates values.
                            if count == 0 && !found_first_nonzero {
                                None
                            } else {
                                found_first_nonzero = true;
                                Some(count)
                            }
                        })
                        .collect();
                    data
                } else {
                    Vec::new()
                };
                let max_bucket = data.iter().max().copied().unwrap_or_default();
                let min_bucket = data.iter().min().copied().unwrap_or_default();
                (
                    data,
                    HistogramMetadata {
                        max_value: histogram.max(),
                        min_value: histogram.min(),
                        max_bucket,
                        min_bucket,
                    },
                )
            })
            .unwrap_or_default()
    }

    /// Get the important percentile values from the histogram
    fn make_percentiles_widget(&self, styles: &view::Styles) -> Text<'static> {
        let mut text = Text::default();
        let histogram = self.poll_times_histogram();
        let percentiles = histogram.iter().flat_map(|histogram| {
            let pairs = [10f64, 25f64, 50f64, 75f64, 90f64, 95f64, 99f64]
                .iter()
                .map(move |i| (*i, histogram.value_at_percentile(*i)));
            pairs.map(|pair| {
                Spans::from(vec![
                    bold(format!("p{:>2}: ", pair.0)),
                    dur(styles, Duration::from_nanos(pair.1)),
                ])
            })
        });
        text.extend(percentiles);
        text
    }
}

fn dur(styles: &view::Styles, dur: std::time::Duration) -> Span<'static> {
    const DUR_PRECISION: usize = 4;
    // TODO(eliza): can we not have to use `format!` to make a string here? is
    // there a way to just give TUI a `fmt::Debug` implementation, or does it
    // have to be given a string in order to do layout stuff?
    styles.time_units(format!("{:.prec$?}", dur, prec = DUR_PRECISION))
}
