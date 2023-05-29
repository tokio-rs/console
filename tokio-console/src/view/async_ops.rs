pub(crate) use crate::view::table::view_controls;
use crate::{
    state::{
        async_ops::{AsyncOp, SortBy},
        resources::Resource,
        Id, State,
    },
    view::{
        self, bold,
        controls::Controls,
        table::{TableList, TableListState},
        DUR_LEN, DUR_TABLE_PRECISION,
    },
};

use tui::{
    layout,
    style::{self, Color, Style},
    text::Spans,
    widgets::{Cell, Row, Table},
};

#[derive(Debug, Default)]
pub(crate) struct AsyncOpsTable {}

pub(crate) struct AsyncOpsTableCtx {
    pub(crate) initial_render: bool,
    pub(crate) resource_id: Id<Resource>,
}

impl TableList<9> for AsyncOpsTable {
    type Row = AsyncOp;
    type Sort = SortBy;
    type Context = AsyncOpsTableCtx;

    const HEADER: &'static [&'static str; 9] = &[
        "ID",
        "Parent",
        "Task",
        "Source",
        "Total",
        "Busy",
        "Idle",
        "Polls",
        "Attributes",
    ];

    const WIDTHS: &'static [usize; 9] = &[
        Self::HEADER[0].len() + 1,
        Self::HEADER[1].len() + 1,
        Self::HEADER[2].len() + 1,
        Self::HEADER[3].len() + 1,
        Self::HEADER[4].len() + 1,
        Self::HEADER[5].len() + 1,
        Self::HEADER[6].len() + 1,
        Self::HEADER[7].len() + 1,
        Self::HEADER[8].len() + 1,
    ];

    fn render<B: tui::backend::Backend>(
        table_list_state: &mut TableListState<Self, 9>,
        styles: &view::Styles,
        frame: &mut tui::terminal::Frame<B>,
        area: layout::Rect,
        state: &mut State,
        ctx: Self::Context,
    ) {
        let now = if let Some(now) = state.last_updated_at() {
            now
        } else {
            // If we have never gotten an update yet, skip...
            return;
        };

        let AsyncOpsTableCtx {
            initial_render,
            resource_id,
        } = ctx;

        if initial_render {
            table_list_state
                .sorted_items
                .extend(state.async_ops_state().async_ops().filter(|op| {
                    op.upgrade()
                        .map(|op| resource_id == op.borrow().resource_id())
                        .unwrap_or(false)
                }))
        } else {
            table_list_state.sorted_items.extend(
                state
                    .async_ops_state_mut()
                    .take_new_async_ops()
                    .filter(|op| {
                        op.upgrade()
                            .map(|op| resource_id == op.borrow().resource_id())
                            .unwrap_or(false)
                    }),
            )
        };
        table_list_state
            .sort_by
            .sort(now, &mut table_list_state.sorted_items);

        let mut id_width = view::Width::new(Self::WIDTHS[0] as u16);
        let mut parent_width = view::Width::new(Self::WIDTHS[1] as u16);
        let mut task_width = view::Width::new(Self::WIDTHS[2] as u16);
        let mut source_width = view::Width::new(Self::WIDTHS[3] as u16);
        let mut polls_width = view::Width::new(Self::WIDTHS[7] as u16);

        let dur_cell = |dur: std::time::Duration| -> Cell<'static> {
            Cell::from(styles.time_units(dur, DUR_TABLE_PRECISION, Some(DUR_LEN)))
        };

        let rows = {
            let id_width = &mut id_width;
            let parent_width = &mut parent_width;
            let task_width = &mut task_width;
            let source_width = &mut source_width;
            let polls_width = &mut polls_width;

            table_list_state
                .sorted_items
                .iter()
                .filter_map(move |async_op| {
                    let async_op = async_op.upgrade()?;
                    let async_op = async_op.borrow();
                    let task_id = async_op.task_id()?;
                    let task = state
                        .tasks_state()
                        .task(task_id)
                        .and_then(|t| t.upgrade())
                        .map(|t| t.borrow().short_desc().to_owned());
                    let task_str = task.unwrap_or_else(|| async_op.task_id_str().to_owned());

                    let mut row = Row::new(vec![
                        Cell::from(id_width.update_str(format!(
                            "{:>width$}",
                            async_op.id(),
                            width = id_width.chars() as usize
                        ))),
                        Cell::from(parent_width.update_str(async_op.parent_id()).to_owned()),
                        Cell::from(task_width.update_str(task_str)),
                        Cell::from(source_width.update_str(async_op.source()).to_owned()),
                        dur_cell(async_op.total(now)),
                        dur_cell(async_op.busy(now)),
                        dur_cell(async_op.idle(now)),
                        Cell::from(polls_width.update_str(async_op.total_polls().to_string())),
                        Cell::from(Spans::from(
                            async_op
                                .formatted_attributes()
                                .iter()
                                .flatten()
                                .cloned()
                                .collect::<Vec<_>>(),
                        )),
                    ]);

                    if async_op.dropped() {
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
            Table::new(rows)
        } else {
            Table::new(rows.rev())
        };

        let block = styles.border_block().title(vec![bold(format!(
            "Async Ops ({}) ",
            table_list_state.len()
        ))]);

        let layout = layout::Layout::default()
            .direction(layout::Direction::Vertical)
            .margin(0);

        let controls = Controls::new(&view_controls(), &area, styles);
        let chunks = layout
            .constraints(
                [
                    layout::Constraint::Length(controls.height()),
                    layout::Constraint::Max(area.height),
                ]
                .as_ref(),
            )
            .split(area);

        let controls_area = chunks[0];
        let async_ops_area = chunks[1];

        let attributes_width = layout::Constraint::Percentage(100);
        let widths = &[
            id_width.constraint(),
            parent_width.constraint(),
            task_width.constraint(),
            source_width.constraint(),
            layout::Constraint::Length(DUR_LEN as u16),
            layout::Constraint::Length(DUR_LEN as u16),
            layout::Constraint::Length(DUR_LEN as u16),
            polls_width.constraint(),
            attributes_width,
        ];

        let table = table
            .header(header)
            .block(block)
            .widths(widths)
            .highlight_symbol(view::TABLE_HIGHLIGHT_SYMBOL)
            .highlight_style(Style::default().add_modifier(style::Modifier::BOLD));

        frame.render_stateful_widget(table, async_ops_area, &mut table_list_state.table_state);
        frame.render_widget(controls.into_widget(), controls_area);

        table_list_state
            .sorted_items
            .retain(|t| t.upgrade().is_some());
    }
}
