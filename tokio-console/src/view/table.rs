use crate::{
    input, state,
    view::{self, bold},
};
use once_cell::sync::OnceCell;
use std::convert::TryFrom;
use tui::{
    layout,
    text::{self, Span, Spans, Text},
    widgets::{Paragraph, TableState, Wrap},
};

use std::cell::RefCell;
use std::rc::Weak;

use super::help::HelpText;

pub(crate) trait TableList<const N: usize> {
    type Row;
    type Sort: SortBy + TryFrom<usize>;
    type Context;

    const HEADER: &'static [&'static str; N];
    const WIDTHS: &'static [usize; N];

    fn render<B: tui::backend::Backend>(
        state: &mut TableListState<Self, N>,
        styles: &view::Styles,
        frame: &mut tui::terminal::Frame<B>,
        area: layout::Rect,
        state: &mut state::State,
        cx: Self::Context,
    ) where
        Self: Sized;
}

pub(crate) trait SortBy {
    fn as_column(&self) -> usize;
}

pub(crate) struct TableListState<T: TableList<N>, const N: usize> {
    pub(crate) sorted_items: Vec<Weak<RefCell<T::Row>>>,
    pub(crate) sort_by: T::Sort,
    pub(crate) selected_column: usize,
    pub(crate) sort_descending: bool,
    pub(crate) table_state: TableState,

    last_key_event: Option<input::KeyEvent>,
}

pub(crate) struct Controls {
    pub(crate) paragraph: Paragraph<'static>,
    pub(crate) height: u16,
}

impl<T: TableList<N>, const N: usize> TableListState<T, N> {
    pub(in crate::view) fn len(&self) -> usize {
        self.sorted_items.len()
    }

    pub(in crate::view) fn update_input(&mut self, event: input::Event) {
        // Clippy likes to remind us that we could use an `if let` here, since
        // the match only has one arm...but this is a `match` because I
        // anticipate adding more cases later...
        #[allow(clippy::single_match)]
        match event {
            input::Event::Key(event) => self.key_input(event),
            _ => {
                // do nothing for now
                // TODO(eliza): mouse input would be cool...
            }
        }
    }

    pub(in crate::view) fn key_input(&mut self, event: input::KeyEvent) {
        use input::KeyCode::*;
        let header_len = T::HEADER.len();
        let code = event.code;
        match code {
            Left | Char('h') => {
                if self.selected_column == 0 {
                    self.selected_column = header_len - 1;
                } else {
                    self.selected_column -= 1;
                }
            }
            Right | Char('l') => {
                if self.selected_column == header_len - 1 {
                    self.selected_column = 0;
                } else {
                    self.selected_column += 1;
                }
            }
            Char('i') => self.sort_descending = !self.sort_descending,
            Down | Char('j') => self.scroll_next(),
            Up | Char('k') => self.scroll_prev(),
            Char('G') => self.scroll_to_last(),
            Char('g') if self.last_key_event.map(|e| e.code) == Some(Char('g')) => {
                self.scroll_to_first()
            }
            _ => {} // do nothing for now...
        }

        if let Ok(sort_by) = T::Sort::try_from(self.selected_column) {
            self.sort_by = sort_by;
        }

        self.last_key_event = Some(event);
    }

    pub(in crate::view) fn scroll_with(
        &mut self,
        f: impl Fn(&Vec<Weak<RefCell<T::Row>>>, usize) -> usize,
    ) {
        // If the list of sorted items is empty, don't try to scroll...
        if self.sorted_items.is_empty() {
            self.table_state.select(None);
            return;
        }

        // Increment the currently selected row, or if no row is selected, start
        // at the first row.
        let i = self.table_state.selected().unwrap_or(0);
        let i = f(&self.sorted_items, i);
        self.table_state.select(Some(i));
    }

    pub(in crate::view) fn scroll_next(&mut self) {
        self.scroll_with(|resources, i| {
            if i >= resources.len() - 1 {
                // If the last itsm is currently selected, wrap around to the
                // first item.
                0
            } else {
                // Otherwise, increase the selected index by 1.
                i + 1
            }
        });
    }

    pub(in crate::view) fn scroll_prev(&mut self) {
        self.scroll_with(|resources, i| {
            if i == 0 {
                // If the first item is currently selected, wrap around to the
                // last item.
                resources.len() - 1
            } else {
                // Otherwise, decrease the selected item by 1.
                i - 1
            }
        })
    }

    pub(in crate::view) fn scroll_to_last(&mut self) {
        self.scroll_with(|resources, _| resources.len() - 1)
    }

    pub(in crate::view) fn scroll_to_first(&mut self) {
        self.scroll_with(|_, _| 0)
    }

    pub(in crate::view) fn selected_item(&self) -> Weak<RefCell<T::Row>> {
        self.table_state
            .selected()
            .map(|i| {
                let selected = if self.sort_descending {
                    i
                } else {
                    self.sorted_items.len() - i - 1
                };
                self.sorted_items[selected].clone()
            })
            .unwrap_or_default()
    }

    pub(in crate::view) fn render<B: tui::backend::Backend>(
        &mut self,
        styles: &view::Styles,
        frame: &mut tui::terminal::Frame<B>,
        area: layout::Rect,
        state: &mut state::State,
        ctx: T::Context,
    ) {
        T::render(self, styles, frame, area, state, ctx)
    }
}

impl<T, const N: usize> Default for TableListState<T, N>
where
    T: TableList<N>,
    T::Sort: Default,
{
    fn default() -> Self {
        let sort_by = T::Sort::default();
        let selected_column = sort_by.as_column();
        Self {
            sorted_items: Default::default(),
            sort_by,
            table_state: Default::default(),
            selected_column,
            sort_descending: false,
            last_key_event: None,
        }
    }
}

impl<T, const N: usize> HelpText for TableListState<T, N>
where
    T: TableList<N>,
{
    fn render_help_content(&self, styles: &view::Styles) -> Paragraph<'static> {
        let controls = vec![
            Spans::from(vec![Span::raw("controls:")]),
            Spans::from(vec![
                Span::raw("  select column (sort): "),
                bold(styles.if_utf8("\u{2190}\u{2192}", "left, right")),
                Span::raw(" or "),
                bold("h, l"),
            ]),
            Spans::from(vec![
                Span::raw("  scroll: "),
                bold(styles.if_utf8("\u{2191}\u{2193}", "up, down")),
                Span::raw(" or "),
                bold("k, j"),
            ]),
            Spans::from(vec![
                Span::raw("  view details: "),
                bold(styles.if_utf8("\u{21B5}", "enter")),
            ]),
            Spans::from(vec![
                text::Span::raw("  invert sort (highest/lowest): "),
                bold("i"),
            ]),
            Spans::from(vec![text::Span::raw("  scroll to top: "), bold("gg")]),
            Spans::from(vec![text::Span::raw("  scroll to bottom: "), bold("G")]),
            Spans::from(vec![text::Span::raw("  toggle help: "), bold("?")]),
            Spans::from(vec![text::Span::raw("  quit: "), bold("q")]),
        ];
        Paragraph::new(controls)
    }
}

struct KeyDisplay {
    base: &'static str,
    utf8: Option<&'static str>,
}

struct ControlDisplay {
    action: &'static str,
    keys: Vec<KeyDisplay>,
}

impl ControlDisplay {
    fn new_simple(action: &'static str, key: &'static str) -> Self {
        ControlDisplay {
            action,
            keys: vec![KeyDisplay {
                base: key,
                utf8: None,
            }],
        }
    }

    fn into_spans(&self, styles: &view::Styles) -> Spans<'static> {
        let mut spans = Vec::new();

        spans.push(Span::from(self.action));
        spans.push(Span::from(" = "));
        let extra_spans: Vec<Span> = self
            .keys
            .iter()
            .map(|kd| {
                Span::from(match kd.utf8 {
                    Some(utf8) => styles.if_utf8(utf8, kd.base),
                    None => kd.base,
                })
            })
            // TODO(hds): somehow I need to intersperse these values with `Span::from(" or ")`.
            .collect();

        Spans::from(spans)
    }
}

fn view_controls() -> Vec<ControlDisplay> {
    vec![
        ControlDisplay {
            action: "select column (sort)",
            keys: vec![
                KeyDisplay {
                    base: "left, right",
                    utf8: Some("\u{2190}\u{2192}"),
                },
                KeyDisplay {
                    base: "h, l",
                    utf8: None,
                },
            ],
        },
        ControlDisplay {
            action: "scroll",
            keys: vec![
                KeyDisplay {
                    base: "up, down",
                    utf8: Some("\u{2191}\u{2193}"),
                },
                KeyDisplay {
                    base: "k, j",
                    utf8: None,
                },
            ],
        },
        ControlDisplay {
            action: "view details",
            keys: vec![KeyDisplay {
                base: "enter",
                utf8: Some("\u{21B5}"),
            }],
        },
        ControlDisplay::new_simple("invert sort (highest/lowest)", "i"),
        ControlDisplay::new_simple("scroll to top", "gg"),
        ControlDisplay::new_simple("scroll to bottom", "G"),
    ]
}

fn universal_controls() -> Vec<ControlDisplay> {
    vec![
        ControlDisplay::new_simple("toggle help", "?"),
        ControlDisplay::new_simple("quit", "q"),
    ]
}

fn controls(styles: &view::Styles) -> Vec<(&str, Spans)> {
    vec![
        (
            "select column (sort)",
            Spans::from(vec![
                bold(styles.if_utf8("\u{2190}\u{2192}", "left, right")),
                Span::raw(" or "),
                bold("h, l"),
            ]),
        ),
        (
            "scroll",
            Spans::from(vec![
                bold(styles.if_utf8("\u{2191}\u{2193}", "up, down")),
                Span::raw(" or "),
                bold("k, j"),
            ]),
        ),
        (
            "view details",
            Spans::from(vec![bold(styles.if_utf8("\u{21B5}", "enter"))]),
        ),
        (
            "invert sort (highest/lowest):",
            Spans::from(vec![bold("i")]),
        ),
        ("scroll to top", Spans::from(vec![bold("gg")])),
        ("scroll to bottom", Spans::from(vec![bold("G")])),
        ("toggle help", Spans::from(vec![bold("?")])),
        ("quit", Spans::from(vec![bold("q")])),
    ]
}

fn build_control_part<'a>(action: &str, keys: Spans<'a>) -> Spans<'a> {
    let mut control = keys;
    let mut label = Spans::from(format!(" = {action}"));
    control.0.append(&mut label.0);

    control
}

impl Controls {
    pub(in crate::view) fn for_area(area: &layout::Rect, styles: &view::Styles) -> Self {
        let mut spans = Spans::from("controls: ");

        let mut controls = controls(styles);
        let quit_control = {
            let (action, keys) = controls.pop().unwrap();
            build_control_part(action, keys)
        };
        let help_control = {
            let (action, keys) = controls.pop().unwrap();
            build_control_part(action, keys)
        };
        //let last_controls
        for (action, keys) in controls {
            let mut next_control = build_control_part(action, keys);
            if spans.width() + next_control.width() + 2 > area.width as usize {
                spans.0.append(&mut next_control.0);
            }
        }

        let text = Text::from(Spans::from(vec![
            Span::raw("controls: "),
            bold(styles.if_utf8("\u{2190}\u{2192}", "left, right")),
            Span::raw(" or "),
            bold("h, l"),
            text::Span::raw(" = select column (sort), "),
            bold(styles.if_utf8("\u{2191}\u{2193}", "up, down")),
            Span::raw(" or "),
            bold("k, j"),
            text::Span::raw(" = scroll, "),
            bold(styles.if_utf8("\u{21B5}", "enter")),
            text::Span::raw(" = view details, "),
            bold("i"),
            text::Span::raw(" = invert sort (highest/lowest), "),
            bold("q"),
            text::Span::raw(" = quit "),
            bold("gg"),
            text::Span::raw(" = scroll to top, "),
            bold("G"),
            text::Span::raw(" = scroll to bottom"),
        ]));

        // how many lines do we need to display the controls?
        let mut height = 1;

        // if the area is narrower than the width of the controls text, we need
        // to wrap the text across multiple lines.
        let width = text.width() as u16;
        if area.width < width {
            height = width / area.width;

            // if the text's width is not neatly divisible by the area's width
            // (and it almost never will be), round up for the remaining text.
            if width % area.width > 0 {
                height += 1
            };
        }

        Self {
            // TODO(eliza): it would be nice if we could wrap this on commas,
            // specifically, rather than whitespace...but that seems like a
            // bunch of additional work...
            paragraph: Paragraph::new(text).wrap(Wrap { trim: true }),
            height,
        }
    }
}
