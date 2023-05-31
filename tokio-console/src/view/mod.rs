use crate::view::help::HelpView;
use crate::view::{
    help::HelpText, resources::ResourcesTable, table::TableListState, tasks::TasksTable,
};
use crate::{input, state::State};
use ratatui::{
    layout,
    style::{self, Style},
    text::Span,
};
use std::{borrow::Cow, cmp};

mod async_ops;
mod controls;
mod durations;
mod help;
mod mini_histogram;
mod percentiles;
mod resource;
mod resources;
mod styles;
mod table;
mod task;
mod tasks;
pub(crate) use self::styles::{Palette, Styles};
pub(crate) use self::table::SortBy;

// This data is only updated every second, so it doesn't make a ton of
// sense to have a lot of precision in timestamps (and this makes sure
// there's room for the unit!)
const DUR_LEN: usize = 6;
// Precision (after decimal point) for durations displayed in a list
// (detail view)
const DUR_LIST_PRECISION: usize = 2;
// Precision (after decimal point) for durations displayed in a table
const DUR_TABLE_PRECISION: usize = 0;
const TABLE_HIGHLIGHT_SYMBOL: &str = ">> ";

pub struct View {
    /// The tasks list is stored separately from the currently selected state,
    /// because it serves as the console's "home screen".
    ///
    /// When we return to the tasks list view (such as by exiting the task
    /// details view), we want to leave the task list's state the way we left it
    /// --- e.g., if the user previously selected a particular sorting, we want
    /// it to remain sorted that way when we return to it.
    tasks_list: TableListState<TasksTable, 12>,
    resources_list: TableListState<ResourcesTable, 9>,
    state: ViewState,
    show_help_modal: bool,
    pub(crate) styles: Styles,
}

pub(crate) enum ViewState {
    /// The table list of all tasks.
    TasksList,
    /// The table list of all resources.
    ResourcesList,
    /// Inspecting a single task instance.
    TaskInstance(self::task::TaskView),
    /// Inspecting a single resource instance.
    ResourceInstance(self::resource::ResourceView),
}

/// The outcome of the update_input method
#[derive(Debug, Copy, Clone)]
pub(crate) enum UpdateKind {
    /// A new task is selected
    SelectTask(u64),
    /// The TaskView is exited
    ExitTaskView,
    /// A new resource is selected
    SelectResource(u64),
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
    (Char($code:literal)) => {
        input::Event::Key(input::KeyEvent {
            code: input::KeyCode::Char($code),
            ..
        })
    };
}

impl View {
    pub fn new(styles: Styles) -> Self {
        Self {
            state: ViewState::TasksList,
            tasks_list: TableListState::<TasksTable, 12>::default(),
            resources_list: TableListState::<ResourcesTable, 9>::default(),
            show_help_modal: false,
            styles,
        }
    }

    pub(crate) fn update_input(&mut self, event: input::Event, state: &State) -> UpdateKind {
        use ViewState::*;
        let mut update_kind = UpdateKind::Other;

        if self.should_toggle_help_modal(&event) {
            self.show_help_modal = !self.show_help_modal;
            return update_kind;
        }

        if matches!(event, key!(Char('t'))) {
            self.state = TasksList;
            return update_kind;
        }

        if matches!(event, key!(Char('r'))) {
            self.state = ResourcesList;
            return update_kind;
        }

        match self.state {
            TasksList => {
                // The enter key changes views, so handle here since we can
                // mutate the currently selected view.
                match event {
                    key!(Enter) => {
                        if let Some(task) = self.tasks_list.selected_item().upgrade() {
                            update_kind = UpdateKind::SelectTask(task.borrow().span_id());
                            self.state = TaskInstance(self::task::TaskView::new(
                                task,
                                state.task_details_ref(),
                            ));
                        }
                    }
                    _ => {
                        // otherwise pass on to view
                        self.tasks_list.update_input(event);
                    }
                }
            }
            ResourcesList => {
                match event {
                    key!(Enter) => {
                        if let Some(res) = self.resources_list.selected_item().upgrade() {
                            update_kind = UpdateKind::SelectResource(res.borrow().span_id());
                            self.state = ResourceInstance(self::resource::ResourceView::new(res));
                        }
                    }
                    _ => {
                        // otherwise pass on to view
                        self.resources_list.update_input(event);
                    }
                }
            }
            ResourceInstance(ref mut view) => {
                // The escape key changes views, so handle here since we can
                // mutate the currently selected view.
                match event {
                    key!(Esc) => {
                        self.state = ResourcesList;
                        update_kind = UpdateKind::Other;
                    }
                    _ => {
                        // otherwise pass on to view
                        view.update_input(event);
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

    /// The help modal should toggle on the `?` key and should exit on `Esc`
    fn should_toggle_help_modal(&mut self, event: &crossterm::event::Event) -> bool {
        input::is_help_toggle(&event) || (self.show_help_modal && input::is_esc(&event))
    }

    pub(crate) fn render<B: ratatui::backend::Backend>(
        &mut self,
        frame: &mut ratatui::terminal::Frame<B>,
        area: layout::Rect,
        state: &mut State,
    ) {
        let help_text: &dyn HelpText = match self.state {
            ViewState::TasksList => {
                self.tasks_list.render(&self.styles, frame, area, state, ());
                &self.tasks_list
            }
            ViewState::ResourcesList => {
                self.resources_list
                    .render(&self.styles, frame, area, state, ());
                &self.resources_list
            }
            ViewState::TaskInstance(ref mut view) => {
                let now = state
                    .last_updated_at()
                    .expect("task view implies we've received an update");
                view.render(&self.styles, frame, area, now);
                view
            }
            ViewState::ResourceInstance(ref mut view) => {
                view.render(&self.styles, frame, area, state);
                view
            }
        };

        state.retain_active();

        if self.show_help_modal {
            let mut help_view = HelpView::new(help_text.render_help_content(&self.styles));
            help_view.render(&self.styles, frame, area, state);
        }
    }

    pub(crate) fn current_view(&self) -> &ViewState {
        &self.state
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
