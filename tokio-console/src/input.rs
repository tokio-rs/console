// TODO(eliza): support Ratatui backends other than crossterm?
// This would probably involve using `spawn_blocking` to drive their blocking
// input-handling mechanisms in the background...
pub use crossterm::event::*;

/// Crossterm on windows reports key release and repeat events which have the
/// effect of duplicating key presses. This function filters out those events.
pub fn should_ignore_key_event(input: &Event) -> bool {
    matches!(
        input,
        Event::Key(KeyEvent {
            kind: KeyEventKind::Release | KeyEventKind::Repeat,
            ..
        })
    )
}

pub fn should_quit(input: &Event) -> bool {
    use Event::*;
    use KeyCode::*;
    match input {
        Key(KeyEvent {
            code: Char('q'), ..
        }) => true,
        Key(KeyEvent {
            code: Char('c'),
            modifiers,
            ..
        })
        | Key(KeyEvent {
            code: Char('d'),
            modifiers,
            ..
        }) if modifiers.contains(KeyModifiers::CONTROL) => true,
        _ => false,
    }
}

pub(crate) fn is_space(input: &Event) -> bool {
    matches!(
        input,
        Event::Key(KeyEvent {
            code: KeyCode::Char(' '),
            ..
        })
    )
}

pub(crate) fn is_help_toggle(event: &Event) -> bool {
    matches!(
        event,
        Event::Key(KeyEvent {
            code: KeyCode::Char('?'),
            ..
        })
    )
}

pub(crate) fn is_esc(event: &Event) -> bool {
    matches!(
        event,
        Event::Key(KeyEvent {
            code: KeyCode::Esc,
            ..
        })
    )
}

#[derive(Debug, Clone)]
pub(crate) enum Event {
    Key(KeyEvent),
    Mouse(MouseEvent),
}

#[derive(Debug, Clone)]
pub(crate) struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyCode {
    Char(char),
    Enter,
    Esc,
    Backspace,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Tab,
    BackTab,
    Delete,
    Insert,
    F(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct KeyModifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub super_: bool,
}

impl Default for KeyModifiers {
    fn default() -> Self {
        Self {
            shift: false,
            control: false,
            alt: false,
            super_: false,
        }
    }
}

pub(crate) fn poll(dur: Duration) -> std::io::Result<Option<Event>> {
    if crossterm::event::poll(dur)? {
        let event = crossterm::event::read()?;
        Ok(Some(convert_event(event)))
    } else {
        Ok(None)
    }
}

fn convert_event(event: Event) -> Event {
    match event {
        Event::Key(key) => Event::Key(KeyEvent {
            code: convert_key_code(key.code),
            modifiers: KeyModifiers {
                shift: key.modifiers.contains(KeyModifiers::shift),
                control: key.modifiers.contains(KeyModifiers::control),
                alt: key.modifiers.contains(KeyModifiers::alt),
                super_: key.modifiers.contains(KeyModifiers::super_),
            },
        }),
        Event::Mouse(mouse) => Event::Mouse(mouse),
        _ => Event::Key(KeyEvent {
            code: KeyCode::Char(' '),
            modifiers: KeyModifiers::default(),
        }),
    }
}

fn convert_key_code(code: KeyCode) -> KeyCode {
    code
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignore_key_repeat_and_release_events() {
        let event = Event::Key(KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        });

        assert!(!should_ignore_key_event(&event));

        let event = Event::Key(KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Release,
            state: KeyEventState::empty(),
        });

        assert!(should_ignore_key_event(&event));

        let event = Event::Key(KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Repeat,
            state: KeyEventState::empty(),
        });

        assert!(should_ignore_key_event(&event));
    }
}
