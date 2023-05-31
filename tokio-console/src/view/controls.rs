use crate::view::{self, bold};

use ratatui::{
    layout,
    text::{Span, Spans, Text},
    widgets::{Paragraph, Widget},
};

/// Construct a widget to display the controls available to the user in the
/// current view.
pub(crate) struct Controls {
    paragraph: Paragraph<'static>,
    height: u16,
}

impl Controls {
    pub(in crate::view) fn new(
        view_controls: &'static [ControlDisplay],
        area: &layout::Rect,
        styles: &view::Styles,
    ) -> Self {
        let universal_controls = universal_controls();

        let mut spans_controls = Vec::with_capacity(view_controls.len() + universal_controls.len());

        spans_controls.extend(view_controls.iter().map(|c| c.to_spans(styles, 0)));
        spans_controls.extend(universal_controls.iter().map(|c| c.to_spans(styles, 0)));

        let mut lines = vec![Spans::from(vec![Span::from("controls: ")])];
        let mut current_line = lines.last_mut().expect("This vector is never empty");
        let separator = Span::from(", ");

        let controls_count: usize = spans_controls.len();
        for (idx, spans) in spans_controls.into_iter().enumerate() {
            // If this is the first item on this line - or first item on the
            // first line, then always include it - even if it goes beyond the
            // line width, not much we can do anyway.
            if idx == 0 || current_line.width() == 0 {
                current_line.0.extend(spans.0);
                continue;
            }

            // Include the width of our separator in the current item if we
            // aren't placing the last item. This is the separator after the
            // new element.
            let needed_trailing_separator_width = if idx == controls_count + 1 {
                separator.width()
            } else {
                0
            };

            let total_width = current_line.width()
                + separator.width()
                + spans.width()
                + needed_trailing_separator_width;

            // If the current item fits on this line, append it.
            // Otherwise, append only the separator - we accounted for its
            // width in the previous loop iteration - and then create a new
            // line for the current item.
            if total_width <= area.width as usize {
                current_line.0.push(separator.clone());
                current_line.0.extend(spans.0);
            } else {
                current_line.0.push(separator.clone());
                lines.push(spans);
                current_line = lines.last_mut().expect("This vector is never empty");
            }
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
    view_controls: &[ControlDisplay],
    styles: &view::Styles,
) -> Paragraph<'a> {
    let universal_controls = universal_controls();

    let mut spans = Vec::with_capacity(1 + view_controls.len() + universal_controls.len());
    spans.push(Spans::from(vec![Span::raw("controls:")]));
    spans.extend(view_controls.iter().map(|c| c.to_spans(styles, 2)));
    spans.extend(universal_controls.iter().map(|c| c.to_spans(styles, 2)));

    Paragraph::new(spans)
}

/// Construct span to display a control.
///
/// A control is made up of an action and one or more keys that will trigger
/// that action.
#[derive(Clone)]
pub(crate) struct ControlDisplay {
    pub(crate) action: &'static str,
    pub(crate) keys: &'static [KeyDisplay],
}

/// A key or keys which will be displayed to the user as part of spans
/// constructed by `ControlDisplay`.
///
/// The `base` description of the key should be ASCII only, more advanced
/// descriptions can be supplied for that key in the `utf8` field. This
/// allows the application to pick the best one to display at runtime
/// based on the termainal being used.
#[derive(Clone)]
pub(crate) struct KeyDisplay {
    pub(crate) base: &'static str,
    pub(crate) utf8: Option<&'static str>,
}

impl ControlDisplay {
    pub(crate) fn to_spans(&self, styles: &view::Styles, indent: usize) -> Spans<'static> {
        let mut spans = Vec::new();

        spans.push(Span::from(" ".repeat(indent)));
        spans.push(Span::from(self.action));
        spans.push(Span::from(" = "));
        for (idx, key_display) in self.keys.iter().enumerate() {
            if idx > 0 {
                spans.push(Span::from(" or "))
            }

            spans.push(bold(match key_display.utf8 {
                Some(utf8) => styles.if_utf8(utf8, key_display.base),
                None => key_display.base,
            }));
        }

        Spans::from(spans)
    }

    // pub(crate) fn into_paragraph()
}

/// Returns a list of controls which are available in all views.
const fn universal_controls() -> &'static [ControlDisplay] {
    &[
        ControlDisplay {
            action: "toggle pause",
            keys: &[KeyDisplay {
                base: "space",
                utf8: None,
            }],
        },
        ControlDisplay {
            action: "quit",
            keys: &[KeyDisplay {
                base: "q",
                utf8: None,
            }],
        },
    ]
}
