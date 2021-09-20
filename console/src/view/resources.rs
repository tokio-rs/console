use crate::{
    state::{
        resources::{Resource, SortBy},
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
    text::Spans,
    widgets::{Cell, Paragraph, Row, Table},
};

#[derive(Debug, Default)]
pub(crate) struct ResourcesTable {}

impl TableList for ResourcesTable {
    type Row = Resource;
    type Sort = SortBy;

    const HEADER: &'static [&'static str] = &[
        "ID",
        "Kind",
        "Total",
        "Target",
        "Type",
        "Location",
        "Attributes",
    ];

    fn render<B: tui::backend::Backend>(
        table_list_state: &mut TableListState<Self>,
        styles: &view::Styles,
        frame: &mut tui::terminal::Frame<B>,
        area: layout::Rect,
        state: &mut State,
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

        let dur_cell = |dur: std::time::Duration| -> Cell<'static> {
            Cell::from(styles.time_units(format!(
                "{:>width$.prec$?}",
                dur,
                width = DUR_LEN,
                prec = DUR_PRECISION,
            )))
        };

        let mut id_width = view::Width::new(Self::HEADER[0].len() as u16);
        let mut kind_width = view::Width::new(Self::HEADER[1].len() as u16);
        let mut target_width = view::Width::new(Self::HEADER[3].len() as u16);
        let mut type_width = view::Width::new(Self::HEADER[4].len() as u16);
        let mut location_width = view::Width::new(Self::HEADER[5].len() as u16);

        let rows = {
            let id_width = &mut id_width;
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
                        Cell::from(kind_width.update_str(resource.kind()).to_owned()),
                        dur_cell(resource.total(now)),
                        Cell::from(target_width.update_str(resource.target()).to_owned()),
                        Cell::from(type_width.update_str(resource.concrete_type()).to_owned()),
                        Cell::from(location_width.update_str(resource.location())),
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
            if idx == table_list_state.selected_column {
                cell.style(selected_style)
            } else {
                cell
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

        let layout = layout::Layout::default()
            .direction(layout::Direction::Vertical)
            .margin(0);

        let chunks = layout
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

        let attributes_width = layout::Constraint::Percentage(100);
        let widths = &[
            id_width.constraint(),
            kind_width.constraint(),
            layout::Constraint::Length(DUR_LEN as u16),
            target_width.constraint(),
            type_width.constraint(),
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
        frame.render_widget(Paragraph::new(table::controls(styles)), controls_area);

        table_list_state
            .sorted_items
            .retain(|t| t.upgrade().is_some());
    }
}
