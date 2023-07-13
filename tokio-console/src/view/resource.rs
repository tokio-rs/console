use crate::{
    input,
    state::resources::Resource,
    state::State,
    view::{
        self,
        async_ops::{self, AsyncOpsTable, AsyncOpsTableCtx},
        bold,
        controls::{controls_paragraph, ControlDisplay, Controls, KeyDisplay},
        help::HelpText,
        TableListState,
    },
};
use once_cell::sync::OnceCell;
use ratatui::{
    layout::{self, Layout},
    text::{Span, Spans, Text},
    widgets::Paragraph,
};
use std::{cell::RefCell, rc::Rc};

pub(crate) struct ResourceView {
    resource: Rc<RefCell<Resource>>,
    pub(crate) async_ops_table: TableListState<AsyncOpsTable, 9>,
    initial_render: bool,
}

impl ResourceView {
    pub(super) fn new(resource: Rc<RefCell<Resource>>) -> Self {
        ResourceView {
            resource,
            async_ops_table: TableListState::<AsyncOpsTable, 9>::default(),
            initial_render: true,
        }
    }

    pub(crate) fn update_input(&mut self, event: input::Event) {
        self.async_ops_table.update_input(event)
    }

    pub(crate) fn render<B: ratatui::backend::Backend>(
        &mut self,
        styles: &view::Styles,
        frame: &mut ratatui::terminal::Frame<B>,
        area: layout::Rect,
        state: &mut State,
    ) {
        let resource = &*self.resource.borrow();
        let controls = Controls::new(view_controls(), &area, styles);

        let (controls_area, stats_area, async_ops_area) = {
            let chunks = Layout::default()
                .direction(layout::Direction::Vertical)
                .constraints(
                    [
                        // controls
                        layout::Constraint::Length(controls.height()),
                        // resource stats
                        layout::Constraint::Length(8),
                        // async ops
                        layout::Constraint::Percentage(60),
                    ]
                    .as_ref(),
                )
                .split(area);
            (chunks[0], chunks[1], chunks[2])
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

        let overview = vec![
            Spans::from(vec![bold("ID: "), Span::raw(resource.id_str())]),
            Spans::from(vec![bold("Parent ID: "), Span::raw(resource.parent())]),
            Spans::from(vec![bold("Kind: "), Span::raw(resource.kind())]),
            Spans::from(vec![bold("Target: "), Span::raw(resource.target())]),
            Spans::from(vec![
                bold("Type: "),
                Span::raw(resource.concrete_type()),
                Span::raw(" "),
                resource.type_visibility().render(styles),
            ]),
            Spans::from(vec![bold("Location: "), Span::raw(resource.location())]),
        ];

        let mut fields = Text::default();
        fields.extend(
            resource
                .formatted_attributes()
                .iter()
                .cloned()
                .map(Spans::from),
        );

        let resource_widget =
            Paragraph::new(overview).block(styles.border_block().title("Resource"));
        let fields_widget = Paragraph::new(fields).block(styles.border_block().title("Attributes"));

        frame.render_widget(controls.into_widget(), controls_area);
        frame.render_widget(resource_widget, stats_area[0]);
        frame.render_widget(fields_widget, stats_area[1]);
        let ctx = AsyncOpsTableCtx {
            initial_render: self.initial_render,
            resource_id: resource.id(),
        };
        self.async_ops_table
            .render(styles, frame, async_ops_area, state, ctx);
        self.initial_render = false;
    }
}

impl HelpText for ResourceView {
    fn render_help_content(&self, styles: &view::Styles) -> Paragraph<'static> {
        controls_paragraph(view_controls(), styles)
    }
}

fn view_controls() -> &'static [ControlDisplay] {
    static VIEW_CONTROLS: OnceCell<Vec<ControlDisplay>> = OnceCell::new();

    VIEW_CONTROLS.get_or_init(|| {
        let resource_controls = &[ControlDisplay {
            action: "return to task list",
            keys: &[KeyDisplay {
                base: "esc",
                utf8: Some("\u{238B} esc"),
            }],
        }];
        [resource_controls, async_ops::view_controls()].concat()
    })
}
