use tui::{
    layout::{self, Constraint, Direction, Layout},
    widgets::{Clear, Paragraph},
};

use crate::{state::State, view};

/// Simple view for help popup
pub(crate) struct HelpView {
    help_text: String,
}

impl HelpView {
    pub(super) fn new(help_text: String) -> Self {
        HelpView { help_text }
    }

    pub(crate) fn render<B: tui::backend::Backend>(
        &mut self,
        styles: &view::Styles,
        frame: &mut tui::terminal::Frame<B>,
        _area: layout::Rect,
        _state: &mut State,
    ) {
        let r = frame.size();
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Percentage(40),
                    Constraint::Percentage(20),
                    Constraint::Percentage(40),
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

        // TODO: This doesn't have to be a clone
        let display_text =
            Paragraph::new(self.help_text.clone()).block(styles.border_block().title("Help"));

        // Clear the help block area and render the popup
        frame.render_widget(Clear, popup_area);
        frame.render_widget(display_text, popup_area);
    }
}
