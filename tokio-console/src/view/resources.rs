use crate::{
    state::{
        resources::{Resource, SortBy},
        State,
    },
    view::{
        self, bold,
        table::{self, TableList, TableListState},
        DUR_LEN, DUR_TABLE_PRECISION,
    },
};

use ratatui::{
    layout,
    style::{self, Color, Style},
    text::Spans,
    widgets::{Cell, Row, Table},
};

#[derive(Debug, Default)]
pub(crate) struct ResourcesTable {}

impl TableList<9> for ResourcesTable {
    type Row = Resource;
    type Sort = SortBy;
    type Context = ();

    const HEADER: &'static [&'static str; 9] = &[
        "ID",
        "Parent",
        "Kind",
        "Total",
        "Target",
        "Type",
        "Vis",
        "Location",
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

    fn render<B: ratatui::backend::Backend>(
        table_list_state: &mut TableListState<Self, 9>,
        styles: &view::Styles,
        frame: &mut ratatui::terminal::Frame<B>,
        area: layout::Rect,
        state: &mut State,
        _: Self::Context,
    ) {
        let now = if let Some(now) = state.last_updated_at() {
            now
        } else {
            // If we have never gotten an update yet, skip...
            return;
        };

        table_list_state
            .sorted_items
            .extend(state.resources_state_mut().take_new_resources());
        table_list_state
            .sort_by
            .sort(now, &mut table_list_state.sorted_items);

        let viz_len: u16 = Self::WIDTHS[6] as u16;

        let mut id_width = view::Width::new(Self::WIDTHS[0] as u16);
        let mut parent_width = view::Width::new(Self::WIDTHS[1] as u16);

        let mut kind_width = view::Width::new(Self::WIDTHS[2] as u16);
        let mut target_width = view::Width::new(Self::WIDTHS[4] as u16);
        let mut type_width = view::Width::new(Self::WIDTHS[5] as u16);
        let mut location_width = view::Width::new(Self::WIDTHS[7] as u16);

        let rows = {
            let id_width = &mut id_width;
            let parent_width = &mut parent_width;
            let kind_width = &mut kind_width;
            let target_width = &mut target_width;
            let type_width = &mut type_width;
            let location_width = &mut location_width;

            table_list_state
                .sorted_items
                .iter()
                .filter_map(move |resource| {
                    let resource = resource.upgrade()?;
                    let resource = resource.borrow();

                    let mut row = Row::new(vec![
                        Cell::from(id_width.update_str(format!(
                            "{:>width$}",
                            resource.id(),
                            width = id_width.chars() as usize
                        ))),
                        Cell::from(parent_width.update_str(resource.parent_id()).to_owned()),
                        Cell::from(kind_width.update_str(resource.kind()).to_owned()),
                        Cell::from(styles.time_units(
                            resource.total(now),
                            DUR_TABLE_PRECISION,
                            Some(DUR_LEN),
                        )),
                        Cell::from(target_width.update_str(resource.target()).to_owned()),
                        Cell::from(type_width.update_str(resource.concrete_type()).to_owned()),
                        Cell::from(resource.type_visibility().render(styles)),
                        Cell::from(location_width.update_str(resource.location()).to_owned()),
                        Cell::from(Spans::from(
                            resource
                                .formatted_attributes()
                                .iter()
                                .flatten()
                                .cloned()
                                .collect::<Vec<_>>(),
                        )),
                    ]);

                    if resource.dropped() {
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
            "Resources ({}) ",
            table_list_state.len()
        ))]);

        let controls = table::Controls::for_area(&area, styles);

        let layout = layout::Layout::default()
            .direction(layout::Direction::Vertical)
            .margin(0);

        let chunks = layout
            .constraints(
                [
                    layout::Constraint::Length(controls.height),
                    layout::Constraint::Max(area.height),
                ]
                .as_ref(),
            )
            .split(area);
        let controls_area = chunks[0];
        let tasks_area = chunks[1];

        let attributes_width = layout::Constraint::Percentage(100);
        let widths = &[
            id_width.constraint(),
            parent_width.constraint(),
            kind_width.constraint(),
            layout::Constraint::Length(DUR_LEN as u16),
            target_width.constraint(),
            type_width.constraint(),
            layout::Constraint::Length(viz_len),
            location_width.constraint(),
            attributes_width,
        ];

        let table = table
            .header(header)
            .block(block)
            .widths(widths)
            .highlight_symbol(view::TABLE_HIGHLIGHT_SYMBOL)
            .highlight_style(Style::default().add_modifier(style::Modifier::BOLD));

        frame.render_stateful_widget(table, tasks_area, &mut table_list_state.table_state);
        frame.render_widget(controls.paragraph, controls_area);

        table_list_state
            .sorted_items
            .retain(|t| t.upgrade().is_some());
    }
}
