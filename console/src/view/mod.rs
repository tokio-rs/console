use crate::{input, tasks::State};
use std::{borrow::Cow, cmp};
use tui::{
    layout,
    style::{self, Style},
    text::Span,
};

mod mini_histogram;
mod task;
mod tasks;

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
                self.list.render(frame, area, tasks);
            }
            ViewState::TaskInstance(ref mut view) => {
                let now = tasks
                    .last_updated_at()
                    .expect("task view implies we've received an update");
                view.render(frame, area, now);
            }
        }

        tasks.retain_active();
    }
}

impl Default for View {
    fn default() -> Self {
        Self {
            state: ViewState::TasksList,
            list: tasks::List::default(),
        }
    }
}

pub(crate) fn bold<'a>(text: impl Into<Cow<'a, str>>) -> Span<'a> {
    Span::styled(text, Style::default().add_modifier(style::Modifier::BOLD))
}

pub(crate) fn color_time_units<'a>(text: impl Into<Cow<'a, str>>) -> Span<'a> {
    use tui::style::Color::Indexed;
    let text = text.into();
    let style = match text.as_ref() {
        s if s.ends_with("ps") => fg_style(Indexed(40)), // green 3
        s if s.ends_with("ns") => fg_style(Indexed(41)), // spring green 3
        s if s.ends_with("µs") || s.ends_with("us") => fg_style(Indexed(42)), // spring green 2
        s if s.ends_with("ms") => fg_style(Indexed(43)), // cyan 3
        s if s.ends_with('s') => fg_style(Indexed(44)),  // dark turquoise,
        _ => Style::default(),
    };
    Span::styled(text, style)
}

fn fg_style(color: style::Color) -> Style {
    Style::default().fg(color)
}

impl Width {
    pub(crate) fn new(curr: u16) -> Self {
        Self { curr }
    }

    pub(crate) fn update_str<S: AsRef<str>>(&mut self, s: S) -> S {
        let len = s.as_ref().len();
        self.curr = cmp::max(self.curr, len as u16);
        s
    }

    pub(crate) fn constraint(&self) -> layout::Constraint {
        layout::Constraint::Length(self.curr)
    }

    pub(crate) fn chars(&self) -> u16 {
        self.curr
    }
}
