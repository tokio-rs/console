use crate::input;
use tui::layout;

mod task;

pub(crate) enum View {
    /// The table list of all tasks.
    TasksList,
    /// Inspecting a single task instance.
    TaskInstance(self::task::TaskView),
}

impl View {
    pub(crate) fn update_input(&mut self, event: input::Event, tasks: &mut crate::tasks::State) {
        match self {
            View::TasksList => {
                // The enter key changes views, so handle here since we can
                // mutate the currently selected view.
                match event {
                    input::Event::Key(input::KeyEvent {
                        code: input::KeyCode::Enter,
                        ..
                    }) => {
                        if let Some(task) = tasks.selected_task().upgrade() {
                            *self = View::TaskInstance(self::task::TaskView::new(task));
                        }
                    }
                    _ => {
                        // otherwise pass on to view
                        tasks.update_input(event);
                    }
                }
            }
            View::TaskInstance(view) => view.update_input(event),
        }
    }

    pub(crate) fn render<B: tui::backend::Backend>(
        &mut self,
        frame: &mut tui::terminal::Frame<B>,
        area: layout::Rect,
        tasks: &mut crate::tasks::State,
    ) {
        match self {
            View::TasksList => {
                tasks.render(frame, area);
                tasks.retain_active();
            }
            View::TaskInstance(view) => {
                view.render(frame, area);
                // retain_active()? should that always be done?
            }
        }
    }
}

impl Default for View {
    fn default() -> Self {
        View::TasksList
    }
}
