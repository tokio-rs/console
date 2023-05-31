// TODO(eliza): support Ratatui backends other than crossterm?
// This would probably involve using `spawn_blocking` to drive their blocking
// input-handling mechanisms in the background...
pub use crossterm::event::*;

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
