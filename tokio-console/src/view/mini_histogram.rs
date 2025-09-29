use std::time::Duration;

use ratatui::{
    layout::{Position, Rect},
    style::Style,
    symbols,
    widgets::{Block, Widget},
};

use crate::state::histogram::DurationHistogram;

/// This is a Ratatui widget to visualize a latency histogram in a small area.
/// It is based on the [`Sparkline`] widget, so it draws a mini bar chart with
/// some labels for clarity. Unlike Sparkline, it does not omit very small
/// values.
///
/// [`Sparkline`]: ratatui::widgets::Sparkline
pub(crate) struct MiniHistogram<'a> {
    /// A block to wrap the widget in
    block: Option<Block<'a>>,
    /// Widget style
    style: Style,
    /// The histogram data to render
    histogram: Option<&'a DurationHistogram>,
    /// The maximum value to take to compute the maximum bar height (if nothing is specified, the
    /// widget uses the max of the dataset)
    max: Option<u64>,
    /// A set of bar symbols used to represent the give data
    bar_set: symbols::bar::Set,
    /// Duration precision for the labels
    duration_precision: usize,
}

#[derive(Debug, Default)]
pub(crate) struct HistogramMetadata {
    /// The max recorded value in the histogram. This is the label for the bottom-right in the chart
    pub(crate) max_value: u64,
    /// The min recorded value in the histogram.
    pub(crate) min_value: u64,
    /// The value of the bucket with the greatest quantity
    pub(crate) max_bucket: u64,
    /// Number of high outliers, if any
    pub(crate) high_outliers: u64,
    pub(crate) highest_outlier: Option<Duration>,
}

impl Default for MiniHistogram<'_> {
    fn default() -> Self {
        MiniHistogram {
            block: None,
            style: Default::default(),
            histogram: None,
            max: None,
            bar_set: symbols::bar::NINE_LEVELS,
            duration_precision: 4,
        }
    }
}

impl Widget for MiniHistogram<'_> {
    fn render(mut self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        let inner_area = match self.block.take() {
            Some(b) => {
                let inner_area = b.inner(area);
                b.render(area, buf);
                inner_area
            }
            None => area,
        };

        if inner_area.height < 1 {
            return;
        }

        let (data, metadata) = match self.histogram {
            // Bit of a deadlock: We cannot know the highest bucket value without determining the number of buckets,
            // and we cannot determine the number of buckets without knowing the width of the chart area which depends on
            // the number of digits in the highest bucket value.
            // So just assume here the number of digits in the highest bucket value is 3.
            // If we overshoot, there will be empty columns/buckets at the right end of the chart.
            // If we undershoot, the rightmost 1-2 columns/buckets will be hidden.
            // We could get the max bucket value from the previous render though...
            Some(h) => chart_data(h, inner_area.width - 3),
            None => return,
        };

        let max_qty_label = metadata.max_bucket.to_string();
        let max_record_label = format!(
            "{:.prec$?}",
            Duration::from_nanos(metadata.max_value),
            prec = self.duration_precision,
        );
        let min_record_label = format!(
            "{:.prec$?}",
            Duration::from_nanos(metadata.min_value),
            prec = self.duration_precision,
        );
        let y_axis_label_width = max_qty_label.len() as u16;

        render_legend(
            inner_area,
            buf,
            &metadata,
            max_record_label,
            min_record_label,
            max_qty_label,
        );

        let legend_height = if metadata.high_outliers > 0 { 2 } else { 1 };

        // Shrink the bars area by 1 row from the bottom
        // and `y_axis_label_width` columns from the left.
        let bars_area = Rect {
            x: inner_area.x + y_axis_label_width,
            y: inner_area.y,
            width: inner_area.width - y_axis_label_width,
            height: inner_area.height - legend_height,
        };
        self.render_bars(bars_area, buf, data);
    }
}

impl<'a> MiniHistogram<'a> {
    fn render_bars(
        &mut self,
        area: ratatui::layout::Rect,
        buf: &mut ratatui::buffer::Buffer,
        data: Vec<u64>,
    ) {
        let max = match self.max {
            Some(v) => v,
            None => *data.iter().max().unwrap_or(&1u64),
        };
        let max_index = std::cmp::min(area.width as usize, data.len());
        let mut data = data
            .iter()
            .take(max_index)
            .map(|e| {
                if max != 0 {
                    let r = e * u64::from(area.height) * 8 / max;
                    // This is the only difference in the bar rendering logic
                    // between MiniHistogram and Sparkline. At least render a
                    // ONE_EIGHT, if the value is greater than 0, even if it's
                    // relatively very small.
                    if *e > 0 && r == 0 {
                        1
                    } else {
                        r
                    }
                } else {
                    0
                }
            })
            .collect::<Vec<u64>>();
        for j in (0..area.height).rev() {
            for (i, d) in data.iter_mut().enumerate() {
                let symbol = match *d {
                    0 => self.bar_set.empty,
                    1 => self.bar_set.one_eighth,
                    2 => self.bar_set.one_quarter,
                    3 => self.bar_set.three_eighths,
                    4 => self.bar_set.half,
                    5 => self.bar_set.five_eighths,
                    6 => self.bar_set.three_quarters,
                    7 => self.bar_set.seven_eighths,
                    _ => self.bar_set.full,
                };

                if let Some(cell) = buf.cell_mut(Position {
                    x: area.left() + i as u16,
                    y: area.top() + j,
                }) {
                    cell.set_symbol(symbol).set_style(self.style);
                }

                if *d > 8 {
                    *d -= 8;
                } else {
                    *d = 0;
                }
            }
        }
    }

    pub fn duration_precision(mut self, precision: usize) -> MiniHistogram<'a> {
        self.duration_precision = precision;
        self
    }

    // The same Sparkline setter methods below

    #[allow(dead_code)]
    pub fn block(mut self, block: Block<'a>) -> MiniHistogram<'a> {
        self.block = Some(block);
        self
    }

    #[allow(dead_code)]
    pub fn style(mut self, style: Style) -> MiniHistogram<'a> {
        self.style = style;
        self
    }

    pub(crate) fn histogram(
        mut self,
        histogram: Option<&'a DurationHistogram>,
    ) -> MiniHistogram<'a> {
        self.histogram = histogram;
        self
    }

    #[allow(dead_code)]
    pub fn max(mut self, max: u64) -> MiniHistogram<'a> {
        self.max = Some(max);
        self
    }

    #[allow(dead_code)]
    pub fn bar_set(mut self, bar_set: symbols::bar::Set) -> MiniHistogram<'a> {
        self.bar_set = bar_set;
        self
    }
}

fn render_legend(
    area: ratatui::layout::Rect,
    buf: &mut ratatui::buffer::Buffer,
    metadata: &HistogramMetadata,
    max_record_label: String,
    min_record_label: String,
    max_qty_label: String,
) {
    // If there are outliers, display a note
    let labels_pos = if metadata.high_outliers > 0 {
        let outliers = format!(
            "{} outliers (highest: {:?})",
            metadata.high_outliers,
            metadata
                .highest_outlier
                .expect("if there are outliers, the highest should be set")
        );
        buf.set_string(
            area.right() - outliers.len() as u16,
            area.bottom() - 1,
            &outliers,
            Style::default(),
        );
        2
    } else {
        1
    };

    // top left: max quantity
    buf.set_string(area.left(), area.top(), &max_qty_label, Style::default());
    // bottom left below the chart: min time
    buf.set_string(
        area.left() + max_qty_label.len() as u16,
        area.bottom() - labels_pos,
        &min_record_label,
        Style::default(),
    );
    // bottom right: max time
    buf.set_string(
        area.right() - max_record_label.len() as u16,
        area.bottom() - labels_pos,
        &max_record_label,
        Style::default(),
    );
}

/// From the histogram, build a visual representation by trying to make as
/// many buckets as the width of the render area.
fn chart_data(histogram: &DurationHistogram, width: u16) -> (Vec<u64>, HistogramMetadata) {
    let &DurationHistogram {
        ref histogram,
        high_outliers,
        highest_outlier,
        ..
    } = histogram;

    let step_size = ((histogram.max() - histogram.min()) as f64 / width as f64).ceil() as u64 + 1;
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
    (
        data,
        HistogramMetadata {
            max_value: histogram.max(),
            min_value: histogram.min(),
            max_bucket,
            high_outliers,
            highest_outlier,
        },
    )
}
