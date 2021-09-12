use std::time::Duration;

use hdrhistogram::Histogram;
use tui::{
    style::Style,
    text::{Span, Spans, Text},
    widgets::{Block, Paragraph, Widget},
};

use crate::view::{bold, styles::DUR_PRECISION};

use super::Styles;

pub(crate) struct Percentiles<'a> {
    block: Option<Block<'a>>,
    style: Style,
    histogram: Option<&'a Histogram<u64>>,
    styles: Option<&'a Styles>,
}

impl<'a> Default for Percentiles<'a> {
    fn default() -> Self {
        Percentiles {
            block: None,
            style: Default::default(),
            histogram: None,
            styles: None,
        }
    }
}

impl<'a> Widget for Percentiles<'a> {
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

        let percentiles = self.histogram.iter().flat_map(|histogram| {
            let pairs = [10f64, 25f64, 50f64, 75f64, 90f64, 95f64, 99f64]
                .iter()
                .map(move |i| (*i, histogram.value_at_percentile(*i)));
            pairs.map(|pair| {
                let dur = if let Some(styles) = self.styles {
                    styles.dur(Duration::from_nanos(pair.1))
                } else {
                    Span::raw(format!("{:.prec$?}", pair.1, prec = DUR_PRECISION))
                };

                Spans::from(vec![bold(format!("p{:>2}: ", pair.0)), dur])
            })
        });

        let mut text = Text::default();
        text.extend(percentiles);
        let paragraph = Paragraph::new(text);
        paragraph.render(inner_area, buf);
    }
}

impl<'a> Percentiles<'a> {
    #[allow(dead_code)]
    pub fn block(mut self, block: Block<'a>) -> Percentiles<'a> {
        self.block = Some(block);
        self
    }

    #[allow(dead_code)]
    pub fn style(mut self, style: Style) -> Percentiles<'a> {
        self.style = style;
        self
    }

    #[allow(dead_code)]
    pub fn histogram(mut self, histogram: &'a Histogram<u64>) -> Percentiles<'a> {
        self.histogram = Some(histogram);
        self
    }

    #[allow(dead_code)]
    pub fn styles(mut self, styles: &'a Styles) -> Percentiles<'a> {
        self.styles = Some(styles);
        self
    }
}
