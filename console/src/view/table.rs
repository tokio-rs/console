use crate::{
    input, state,
    view::{self, bold},
};
use std::convert::TryFrom;
use tui::{
    layout,
    text::{self, Span, Spans, Text},
    widgets::TableState,
};

use std::cell::RefCell;
use std::rc::Weak;

pub(crate) trait TableList {
    type Row;
    type Sort: SortBy + TryFrom<usize>;
    type Context;

    const HEADER: &'static [&'static str];

    fn render<B: tui::backend::Backend>(
        state: &mut TableListState<Self>,
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

pub(crate) struct TableListState<T: TableList> {
    pub(crate) sorted_items: Vec<Weak<RefCell<T::Row>>>,
    pub(crate) sort_by: T::Sort,
    pub(crate) selected_column: usize,
    pub(crate) sort_descending: bool,
    pub(crate) table_state: TableState,
}

impl<T: TableList> TableListState<T> {
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

    pub(in crate::view) fn key_input(&mut self, input::KeyEvent { code, .. }: input::KeyEvent) {
        use input::KeyCode::*;
        let header_len = T::HEADER.len();
        match code {
            Left => {
                if self.selected_column == 0 {
                    self.selected_column = header_len - 1;
                } else {
                    self.selected_column -= 1;
                }
            }
            Right => {
                if self.selected_column == header_len - 1 {
                    self.selected_column = 0;
                } else {
                    self.selected_column += 1;
                }
            }
            Char('i') => self.sort_descending = !self.sort_descending,
            Down => self.scroll_next(),
            Up => self.scroll_prev(),
            _ => {} // do nothing for now...
        }

        if let Ok(sort_by) = T::Sort::try_from(self.selected_column) {
            self.sort_by = sort_by;
        }
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

pub(in crate::view) fn controls(styles: &view::Styles) -> Text {
    tui::text::Text::from(Spans::from(vec![
        Span::raw("controls: "),
        bold(styles.if_utf8("\u{2190}\u{2192}", "left, right")),
        text::Span::raw(" = select column (sort), "),
        bold(styles.if_utf8("\u{2191}\u{2193}", "up, down")),
        text::Span::raw(" = scroll, "),
        bold(styles.if_utf8("\u{21B5}", "enter")),
        text::Span::raw(" = view details, "),
        bold("i"),
        text::Span::raw(" = invert sort (highest/lowest), "),
        bold("q"),
        text::Span::raw(" = quit"),
    ]))
}

impl<T> Default for TableListState<T>
where
    T: TableList,
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
        }
    }
}
