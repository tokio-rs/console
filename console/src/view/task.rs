use crate::{
    input,
    tasks::{Details, DetailsRef, Task},
    view::bold,
};
use std::{cell::RefCell, rc::Rc, time::SystemTime};
use tui::{
    layout::{self, Layout},
    text::{Span, Spans, Text},
    widgets::{Block, Borders, Paragraph, Sparkline},
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
        const DUR_PRECISION: usize = 4;

        let chunks = Layout::default()
            .direction(layout::Direction::Vertical)
            .constraints(
                [
                    layout::Constraint::Length(1),
                    layout::Constraint::Length(6),
                    layout::Constraint::Length(6),
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

        let histogram_area = Layout::default()
            .direction(layout::Direction::Horizontal)
            .constraints(
                [
                    layout::Constraint::Percentage(50),
                    layout::Constraint::Percentage(50),
                ]
                .as_ref(),
            )
            .split(chunks[2]);

        let percentiles_columns = Layout::default()
            .direction(layout::Direction::Horizontal)
            .constraints(
                [
                    layout::Constraint::Percentage(50),
                    layout::Constraint::Percentage(50),
                ]
                .as_ref(),
            )
            .split(histogram_area[0].inner(&layout::Margin {
                horizontal: 1,
                vertical: 1,
            }));

        let fields_area = chunks[3];
        let sparkline_area = histogram_area[1];

        let controls = Spans::from(vec![
            Span::raw("controls: "),
            bold("esc"),
            Span::raw(" = return to task list, "),
            bold("q"),
            Span::raw(" = quit"),
        ]);

        let attrs = Spans::from(vec![bold("ID: "), Span::raw(task.id_hex())]);

        let mut total = vec![
            bold("Total Time: "),
            Span::from(format!("{:.prec$?}", task.total(now), prec = DUR_PRECISION,)),
        ];

        // TODO(eliza): maybe surface how long the task has been completed, as well?
        if task.is_completed() {
            total.push(Span::raw(" (completed)"));
        };

        let total = Spans::from(total);

        let busy = Spans::from(vec![
            bold("Busy: "),
            Span::from(format!("{:.prec$?}", task.busy(), prec = DUR_PRECISION,)),
        ]);
        let idle = Spans::from(vec![
            bold("Idle: "),
            Span::from(format!("{:.prec$?}", task.idle(now), prec = DUR_PRECISION,)),
        ]);

        let metrics = vec![attrs, total, busy, idle];

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

        fn block_for(title: &str) -> Block {
            Block::default().borders(Borders::ALL).title(title)
        }

        let mut fields = Text::default();
        fields.extend(task.formatted_fields().iter().cloned().map(Spans::from));

        let chart_data = make_chart_data(self.details.clone(), task.id(), sparkline_area.width - 2);
        let histogram_sparkline = Sparkline::default()
            .block(Block::default().title("Poll Times").borders(Borders::ALL))
            .data(&chart_data);

        let task_widget = Paragraph::new(metrics).block(block_for("Task"));
        let wakers_widget = Paragraph::new(vec![wakers, wakeups]).block(block_for("Waker"));
        let fields_widget = Paragraph::new(fields).block(block_for("Fields"));

        let percentiles_widget = block_for("Poll Times Stats");
        let (percentiles_1, percentiles_2) =
            make_percentiles_widgets(self.details.clone(), task.id());
        let percentiles_widget_1 = Paragraph::new(percentiles_1);
        let percentiles_widget_2 = Paragraph::new(percentiles_2);

        frame.render_widget(Block::default().title(controls), controls_area);
        frame.render_widget(task_widget, stats_area[0]);
        frame.render_widget(wakers_widget, stats_area[1]);
        frame.render_widget(fields_widget, fields_area);
        frame.render_widget(percentiles_widget, histogram_area[0]);
        frame.render_widget(percentiles_widget_1, percentiles_columns[0]);
        frame.render_widget(percentiles_widget_2, percentiles_columns[1]);
        frame.render_widget(histogram_sparkline, sparkline_area);
    }
}

fn make_chart_data(details: DetailsRef, task_id: u64, width: u16) -> Vec<u64> {
    details
        .borrow()
        .as_ref()
        .and_then(|details| filter_same_task(task_id, details))
        .and_then(|details| details.poll_times_histogram())
        .map(|histogram| {
            // This is probably very buggy
            let steps = ((histogram.max() - histogram.min()) as f64 / width as f64).ceil() as u64;
            if steps > 0 {
                let data: Vec<u64> = histogram
                    .iter_linear(steps)
                    .map(|it| it.count_since_last_iteration())
                    .collect();
                data
            } else {
                Vec::new()
            }
        })
        .unwrap_or_default()
}

fn make_percentiles_widgets(details: DetailsRef, task_id: u64) -> (Text<'static>, Text<'static>) {
    let percentiles_iter = details
        .borrow()
        .as_ref()
        .and_then(|details| filter_same_task(task_id, details))
        .and_then(|details| details.poll_times_histogram())
        .map(|histogram| {
            vec![
                (10, histogram.value_at_percentile(10.0)),
                (25, histogram.value_at_percentile(25.0)),
                (50, histogram.value_at_percentile(50.0)),
                (75, histogram.value_at_percentile(75.0)),
                (90, histogram.value_at_percentile(90.0)),
                (95, histogram.value_at_percentile(95.0)),
                (99, histogram.value_at_percentile(99.0)),
            ]
        })
        .map(|pairs| {
            pairs
                .into_iter()
                .map(|pair| format!("p{}: {}ms", pair.0, (pair.1 as f64 / 1000f64)))
        });

    let mut percentiles_1 = Text::default();
    let mut percentiles_2 = Text::default();
    if let Some(mut percentiles_iter) = percentiles_iter {
        percentiles_1.extend(percentiles_iter.by_ref().take(4).map(Spans::from));
        percentiles_2.extend(percentiles_iter.map(Spans::from));
    }
    (percentiles_1, percentiles_2)
}

fn filter_same_task(task_id: u64, details: &Details) -> Option<&Details> {
    if details.task_id() == task_id {
        Some(details)
    } else {
        None
    }
}
