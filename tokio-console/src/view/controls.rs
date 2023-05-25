use crate::view::{self, bold};

use once_cell::sync::OnceCell;
use tui::{
    layout,
    text::{Span, Spans, Text},
    widgets::{Paragraph, Widget},
};

pub(crate) struct Controls {
    paragraph: Paragraph<'static>,
    height: u16,
}

impl Controls {
    pub(in crate::view) fn new(
        view_controls: &Vec<ControlDisplay>,
        area: &layout::Rect,
        styles: &view::Styles,
    ) -> Self {
        let universal_controls = universal_controls();

        let mut spans_controls = Vec::with_capacity(view_controls.len() + universal_controls.len());
        spans_controls.extend(view_controls.iter().map(|c| c.into_spans(styles, 0)));
        spans_controls.extend(universal_controls.iter().map(|c| c.into_spans(styles, 0)));

        let mut lines = vec![Spans::from(vec![Span::from("controls: ")])];
        let mut current_line = lines.last_mut().expect("This vector is never empty");
        let separator = Span::from(", ");

        let mut idx = 0;
        let controls_count = spans_controls.len();
        for spans in spans_controls {
            if idx == 0 || current_line.width() == 0 {
                current_line.0.extend(spans.0);
            } else {
                let needed_trailing_separator_width = if idx == controls_count + 1 {
                    separator.width()
                } else {
                    0
                };

                if current_line.width()
                    + separator.width()
                    + spans.width()
                    + needed_trailing_separator_width
                    <= area.width as usize
                {
                    current_line.0.push(separator.clone());
                    current_line.0.extend(spans.0);
                } else {
                    current_line.0.push(separator.clone());
                    lines.push(spans);
                    current_line = lines.last_mut().expect("This vector is never empty");
                }
            }
            idx += 1;
        }

        let height = lines.len() as u16;
        let text = Text::from(lines);

        Self {
            paragraph: Paragraph::new(text),
            height,
        }
    }

    pub(crate) fn height(&self) -> u16 {
        self.height
    }

    pub(crate) fn into_widget(self) -> impl Widget {
        self.paragraph
    }
}

pub(crate) fn controls_paragraph<'a>(
    view_controls: &Vec<ControlDisplay>,
    styles: &view::Styles,
) -> Paragraph<'a> {
    let universal_controls = universal_controls();

    let mut spans = Vec::with_capacity(1 + view_controls.len() + universal_controls.len());
    spans.push(Spans::from(vec![Span::raw("controls:")]));
    spans.extend(view_controls.iter().map(|c| c.into_spans(styles, 2)));
    spans.extend(universal_controls.iter().map(|c| c.into_spans(styles, 2)));

    Paragraph::new(spans)
}

pub(crate) struct KeyDisplay {
    pub(crate) base: &'static str,
    pub(crate) utf8: Option<&'static str>,
}

pub(crate) struct ControlDisplay {
    pub(crate) action: &'static str,
    pub(crate) keys: Vec<KeyDisplay>,
}

impl ControlDisplay {
    pub(crate) fn new_simple(action: &'static str, key: &'static str) -> Self {
        ControlDisplay {
            action,
            keys: vec![KeyDisplay {
                base: key,
                utf8: None,
            }],
        }
    }

    pub(crate) fn into_spans(&self, styles: &view::Styles, indent: usize) -> Spans<'static> {
        let mut spans = Vec::new();

        spans.push(Span::from(
            std::iter::repeat(" ").take(indent).collect::<String>(),
        ));
        spans.push(Span::from(self.action));
        spans.push(Span::from(" = "));
        for (idx, key_display) in self.keys.iter().enumerate() {
            if idx > 0 {
                spans.push(Span::from(" or "))
            }
            spans.push(Span::from(bold(match key_display.utf8 {
                Some(utf8) => styles.if_utf8(utf8, key_display.base),
                None => key_display.base,
            })));
        }

        Spans::from(spans)
    }

    // pub(crate) fn into_paragraph()
}

pub(crate) fn universal_controls() -> &'static Vec<ControlDisplay> {
    static UNIVERSAL_CONTROLS: OnceCell<Vec<ControlDisplay>> = OnceCell::new();

    UNIVERSAL_CONTROLS.get_or_init(|| {
        vec![
            ControlDisplay::new_simple("toggle pause", "space"),
            ControlDisplay::new_simple("toggle help", "?"),
            ControlDisplay::new_simple("quit", "q"),
        ]
    })
}
