use std::cmp;

use tui::{
    layout::{self},
    widgets::Widget,
};

use crate::{
    state::histogram::DurationHistogram,
    view::{self, mini_histogram::MiniHistogram, percentiles::Percentiles},
};

// This is calculated so that a legend like the below generally fits:
// │0647.17µs  909.31µs │
// This also gives at characters for the sparkline itself.
const MIN_HISTOGRAM_BLOCK_WIDTH: u16 = 22;

/// This is a tui-rs widget to visualize durations as a list of percentiles
/// and if possible, a mini-histogram too.
///
/// This widget wraps the [`Percentiles`] and [`MiniHistogram`] widgets which
/// are displayed side by side. The mini-histogram will only be displayed if
///   a) UTF-8 support is enabled via [`Styles`]
///   b) There is at least a minimum width (22 characters to display the full
///      bottom legend) left after drawing the percentiles
///
/// This
///
/// [`Styles`]: crate::view::Styles
pub(crate) struct Durations<'a> {
    /// Widget style
    styles: &'a view::Styles,
    /// The histogram data to render
    histogram: Option<&'a DurationHistogram>,
    /// Title for percentiles block
    percentiles_title: &'a str,
    /// Title for histogram sparkline block
    histogram_title: &'a str,
}

impl<'a> Widget for Durations<'a> {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        // Only split the durations area in half if we're also drawing a
        // sparkline. We require UTF-8 to draw the sparkline and also enough width.
        let (percentiles_area, histogram_area) = if self.styles.utf8 {
            let percentiles_width = cmp::max(self.percentiles_title.len() as u16, 13_u16) + 2;

            // If there isn't enough width left after drawing the percentiles
            // then we won't draw the sparkline at all.
            if area.width < percentiles_width + MIN_HISTOGRAM_BLOCK_WIDTH {
                (area, None)
            } else {
                let areas = layout::Layout::default()
                    .direction(layout::Direction::Horizontal)
                    .constraints(
                        [
                            layout::Constraint::Length(percentiles_width),
                            layout::Constraint::Min(MIN_HISTOGRAM_BLOCK_WIDTH),
                        ]
                        .as_ref(),
                    )
                    .split(area);
                (areas[0], Some(areas[1]))
            }
        } else {
            (area, None)
        };

        let percentiles_widget = Percentiles::new(self.styles)
            .title(self.percentiles_title)
            .histogram(self.histogram);
        percentiles_widget.render(percentiles_area, buf);

        if let Some(histogram_area) = histogram_area {
            let histogram_widget = MiniHistogram::default()
                .block(self.styles.border_block().title(self.histogram_title))
                .histogram(self.histogram)
                .duration_precision(2);
            histogram_widget.render(histogram_area, buf);
        }
    }
}

impl<'a> Durations<'a> {
    pub(crate) fn new(styles: &'a view::Styles) -> Self {
        Self {
            styles,
            histogram: None,
            percentiles_title: "Percentiles",
            histogram_title: "Histogram",
        }
    }

    pub(crate) fn histogram(mut self, histogram: Option<&'a DurationHistogram>) -> Self {
        self.histogram = histogram;
        self
    }

    pub(crate) fn percentiles_title(mut self, title: &'a str) -> Self {
        self.percentiles_title = title;
        self
    }

    pub(crate) fn histogram_title(mut self, title: &'a str) -> Self {
        self.histogram_title = title;
        self
    }
}
