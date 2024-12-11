use std::time::Duration;

use ratatui::{
    text::{Line, Text},
    widgets::{Paragraph, Widget},
};

use crate::{
    state::histogram::DurationHistogram,
    view::{self, bold},
};

/// This is a Ratatui widget to display duration percentiles in a list form.
/// It wraps the [`Paragraph`] widget.
pub(crate) struct Percentiles<'a> {
    /// Widget style
    styles: &'a view::Styles,
    /// The histogram data to render
    histogram: Option<&'a DurationHistogram>,
    /// The title of the paragraph
    title: &'a str,
}

impl Widget for Percentiles<'_> {
    fn render(self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        let inner = Paragraph::new(self.make_percentiles_inner())
            .block(self.styles.border_block().title(self.title));

        inner.render(area, buf)
    }
}

impl<'a> Percentiles<'a> {
    pub(crate) fn new(styles: &'a view::Styles) -> Self {
        Self {
            styles,
            histogram: None,
            title: "Percentiles",
        }
    }

    pub(crate) fn make_percentiles_inner(&self) -> Text<'static> {
        let mut text = Text::default();
        let histogram = match self.histogram {
            Some(DurationHistogram { histogram, .. }) => histogram,
            _ => return text,
        };

        // Get the important percentile values from the histogram
        let pairs = [10f64, 25f64, 50f64, 75f64, 90f64, 95f64, 99f64]
            .iter()
            .map(move |i| (*i, histogram.value_at_percentile(*i)));
        let percentiles = pairs.map(|pair| {
            Line::from(vec![
                bold(format!("p{:>2}: ", pair.0)),
                self.styles.time_units(
                    Duration::from_nanos(pair.1),
                    view::DUR_LIST_PRECISION,
                    None,
                ),
            ])
        });

        text.extend(percentiles);
        text
    }

    #[allow(dead_code)]
    pub(crate) fn styles(mut self, styles: &'a view::Styles) -> Percentiles<'a> {
        self.styles = styles;
        self
    }

    pub(crate) fn histogram(mut self, histogram: Option<&'a DurationHistogram>) -> Percentiles<'a> {
        self.histogram = histogram;
        self
    }

    pub(crate) fn title(mut self, title: &'a str) -> Percentiles<'a> {
        self.title = title;
        self
    }
}
