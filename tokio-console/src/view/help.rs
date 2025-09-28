use ratatui::{
    layout::{self, Constraint, Direction, Layout},
    widgets::{Clear, Paragraph},
};

use crate::{state::State, view};

pub(crate) trait HelpText {
    fn render_help_content(&self, styles: &view::Styles) -> Paragraph<'static>;
}

/// Simple view for help popup
pub(crate) struct HelpView<'a> {
    help_text: Option<Paragraph<'a>>,
}

impl<'a> HelpView<'a> {
    pub(super) fn new(help_text: Paragraph<'a>) -> Self {
        HelpView {
            help_text: Some(help_text),
        }
    }

    pub(crate) fn render(
        &mut self,
        styles: &view::Styles,
        frame: &mut ratatui::Frame,
        _area: layout::Rect,
        _state: &mut State,
    ) {
        let r = frame.area();
        let content = self
            .help_text
            .take()
            .expect("help_text should be initialized");

        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Percentage(20),
                    Constraint::Min(15),
                    Constraint::Percentage(20),
                ]
                .as_ref(),
            )
            .split(r);

        let popup_area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(
                [
                    Constraint::Percentage(20),
                    Constraint::Percentage(60),
                    Constraint::Percentage(20),
                ]
                .as_ref(),
            )
            .split(popup_layout[1])[1];

        let display_text = content.block(styles.border_block().title("Help"));

        // Clear the help block area and render the popup
        frame.render_widget(Clear, popup_area);
        frame.render_widget(display_text, popup_area);
    }
}
