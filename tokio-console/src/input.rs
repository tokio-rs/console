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
