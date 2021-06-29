use std::time::Duration;

use tui::{
    layout::Rect,
    style::Style,
    symbols,
    widgets::{Block, Widget},
};

/// This is a tui-rs widget to visualize a latency histogram in a small area.
/// It is based on the [`Sparkline`] widget, so it draws a mini bar chart with
/// some labels for clarity. Unlike Sparkline, it does not omit very small values.
pub(crate) struct MiniHistogram<'a> {
    /// A block to wrap the widget in
    block: Option<Block<'a>>,
    /// Widget style
    style: Style,
    /// Values for the buckets of the histogram
    data: &'a [u64],
    /// Metadata about the histogram
    metadata: HistogramMetadata,
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
    /// The value of the bucket with the smallest quantity
    pub(crate) min_bucket: u64,
}

impl<'a> Default for MiniHistogram<'a> {
    fn default() -> Self {
        MiniHistogram {
            block: None,
            style: Default::default(),
            data: &[],
            metadata: Default::default(),
            max: None,
            bar_set: symbols::bar::NINE_LEVELS,
            duration_precision: 4,
        }
    }
}

impl<'a> Widget for MiniHistogram<'a> {
    fn render(mut self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
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

        let max_qty_label = self.metadata.max_bucket.to_string();
        let min_qty_label = self.metadata.min_bucket.to_string();
        let max_record_label = format!(
            "{:.prec$?}",
            Duration::from_nanos(self.metadata.max_value),
            prec = self.duration_precision,
        );
        let min_record_label = format!(
            "{:.prec$?}",
            Duration::from_nanos(self.metadata.min_value),
            prec = self.duration_precision,
        );
        let y_axis_label_width = max_qty_label.len() as u16;

        self.render_legend(
            inner_area,
            buf,
            max_record_label,
            min_record_label,
            max_qty_label,
            min_qty_label,
        );

        // Shrink the bars area by 1 row from the bottom
        // and `y_axis_label_width` columns from the left.
        let bars_area = Rect {
            x: inner_area.x + y_axis_label_width,
            y: inner_area.y,
            width: inner_area.width - y_axis_label_width,
            height: inner_area.height - 1,
        };
        self.render_bars(bars_area, buf);
    }
}

impl<'a> MiniHistogram<'a> {

    fn render_legend(
        &mut self,
        area: tui::layout::Rect,
        buf: &mut tui::buffer::Buffer,
        max_record_label: String,
        min_record_label: String,
        max_qty_label: String,
        min_qty_label: String,
    ) {
        // top left: max quantity
        buf.set_string(area.left(), area.top(), &max_qty_label, Style::default());
        // bottom left: 0 aligned to right
        let zero_label = format!("{:>width$}", &min_qty_label, width = max_qty_label.len());
        buf.set_string(
            area.left(),
            area.bottom() - 2,
            &zero_label,
            Style::default(),
        );
        // bottom left below the chart: min time
        buf.set_string(
            area.left() + max_qty_label.len() as u16,
            area.bottom() - 1,
            &min_record_label,
            Style::default(),
        );
        // bottom right: max time
        buf.set_string(
            area.right() - max_record_label.len() as u16,
            area.bottom() - 1,
            &max_record_label,
            Style::default(),
        );
    }

    fn render_bars(&mut self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let max = match self.max {
            Some(v) => v,
            None => *self.data.iter().max().unwrap_or(&1u64),
        };
        let max_index = std::cmp::min(area.width as usize, self.data.len());
        let mut data = self
            .data
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
                buf.get_mut(area.left() + i as u16, area.top() + j)
                    .set_symbol(symbol)
                    .set_style(self.style);

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

    #[allow(dead_code)]
    pub fn data(mut self, data: &'a [u64]) -> MiniHistogram<'a> {
        self.data = data;
        self
    }

    #[allow(dead_code)]
    pub fn metadata(mut self, metadata: HistogramMetadata) -> MiniHistogram<'a> {
        self.metadata = metadata;
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
