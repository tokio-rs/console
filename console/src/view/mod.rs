use crate::{input, tasks::State};
use std::{borrow::Cow, cmp};
use tui::{
    layout,
    style::{self, Style},
    text::Span,
};

mod mini_histogram;
mod styles;
mod task;
mod tasks;
pub(crate) use self::styles::{Palette, Styles};

pub struct View {
    /// The tasks list is stored separately from the currently selected state,
    /// because it serves as the console's "home screen".
    ///
    /// When we return to the tasks list view (such as by exiting the task
    /// details view), we want to leave the task list's state the way we left it
    /// --- e.g., if the user previously selected a particular sorting, we want
    /// it to remain sorted that way when we return to it.
    list: tasks::List,
    state: ViewState,
    pub(crate) styles: Styles,
}

enum ViewState {
    /// The table list of all tasks.
    TasksList,
    /// Inspecting a single task instance.
    TaskInstance(self::task::TaskView),
}

/// The outcome of the update_input method
#[derive(Debug, Copy, Clone)]
pub(crate) enum UpdateKind {
    /// A new task is selected
    SelectTask(u64),
    /// The TaskView is exited
    ExitTaskView,
    /// No significant change
    Other,
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct Width {
    curr: u16,
}

macro_rules! key {
    ($code:ident) => {
        input::Event::Key(input::KeyEvent {
            code: input::KeyCode::$code,
            ..
        })
    };
}

impl View {
    pub fn new(styles: Styles) -> Self {
        Self {
            state: ViewState::TasksList,
            list: tasks::List::default(),
            styles,
        }
    }

    pub(crate) fn update_input(&mut self, event: input::Event, tasks: &State) -> UpdateKind {
        use ViewState::*;
        let mut update_kind = UpdateKind::Other;
        match self.state {
            TasksList => {
                // The enter key changes views, so handle here since we can
                // mutate the currently selected view.
                match event {
                    key!(Enter) => {
                        if let Some(task) = self.list.selected_task().upgrade() {
                            update_kind = UpdateKind::SelectTask(task.borrow().id());
                            self.state =
                                TaskInstance(self::task::TaskView::new(task, tasks.details_ref()));
                        }
                    }
                    _ => {
                        // otherwise pass on to view
                        self.list.update_input(event);
                    }
                }
            }
            TaskInstance(ref mut view) => {
                // The escape key changes views, so handle here since we can
                // mutate the currently selected view.
                match event {
                    key!(Esc) => {
                        self.state = TasksList;
                        update_kind = UpdateKind::ExitTaskView;
                    }
                    _ => {
                        // otherwise pass on to view
                        view.update_input(event);
                    }
                }
            }
        }
        update_kind
    }

    pub(crate) fn render<B: tui::backend::Backend>(
        &mut self,
        frame: &mut tui::terminal::Frame<B>,
        area: layout::Rect,
        tasks: &mut crate::tasks::State,
    ) {
        match self.state {
            ViewState::TasksList => {
                self.list.render(&self.styles, frame, area, tasks);
            }
            ViewState::TaskInstance(ref mut view) => {
                let now = tasks
                    .last_updated_at()
                    .expect("task view implies we've received an update");
                view.render(&self.styles, frame, area, now);
            }
        }

        tasks.retain_active();
    }
}

pub(crate) fn bold<'a>(text: impl Into<Cow<'a, str>>) -> Span<'a> {
    Span::styled(text, Style::default().add_modifier(style::Modifier::BOLD))
}

impl Width {
    pub(crate) fn new(curr: u16) -> Self {
        Self { curr }
    }

    pub(crate) fn update_str<S: AsRef<str>>(&mut self, s: S) -> S {
        self.update_len(s.as_ref().len());
        s
    }
    pub(crate) fn update_len(&mut self, len: usize) {
        let max = cmp::max(self.curr as usize, len);
        // Cap since a string could be stupid-long and not fit in a u16.
        // 100 is arbitrarily chosen, to keep the UI sane.
        self.curr = cmp::min(max, 100) as u16;
    }

    pub(crate) fn constraint(&self) -> layout::Constraint {
        layout::Constraint::Length(self.curr)
    }

    pub(crate) fn chars(&self) -> u16 {
        self.curr
    }
}
