// SPDX-License-Identifier: GPL-3.0

use cosmic::iced::keyboard::{self, key::Named, Key, Modifiers};
use widgets::text_editor::{Binding, KeyPress, Motion, Status};

use crate::app::{Message, VimAction};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VimMode {
    #[default]
    Normal,
    Insert,
    Visual,
    VisualLine,
}

impl VimMode {
    pub fn label(&self) -> &'static str {
        match self {
            VimMode::Normal => "-- NORMAL --",
            VimMode::Insert => "-- INSERT --",
            VimMode::Visual => "-- VISUAL --",
            VimMode::VisualLine => "-- VISUAL LINE --",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VimPendingOperator {
    G, // 'g' pressed, waiting for 'g' (gg)
    Y, // 'y' pressed in Normal mode, waiting for 'y' (yy)
    D, // 'd' pressed in Normal mode, waiting for 'd' (dd)
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VimState {
    pub mode: VimMode,
    pub count_prefix: Option<usize>,
    pub pending_operator: Option<VimPendingOperator>,
    pub last_yank_is_line: bool,
}

impl VimState {
    pub fn reset_operator_and_count(&mut self) {
        self.pending_operator = None;
        self.count_prefix = None;
    }
}

/// Translates a key press event in Vim mode into a text editor [`Binding`].
pub fn handle_vim_key_press(
    vim: &mut VimState,
    key_press: &KeyPress,
) -> Option<Binding<Message>> {
    let KeyPress {
        key,
        modified_key,
        physical_key,
        modifiers,
        status,
        ..
    } = key_press;

    if !matches!(status, Status::Focused { .. }) {
        return None;
    }

    // Always intercept Escape in Vim mode so the editor remains focused and returns to Normal mode.
    if matches!(modified_key, Key::Named(Named::Escape))
        || (modifiers.control() && matches!(key.to_latin(*physical_key), Some('[')))
    {
        vim.reset_operator_and_count();
        let was_insert = vim.mode == VimMode::Insert;
        vim.mode = VimMode::Normal;

        return if was_insert {
            Some(Binding::Sequence(vec![
                Binding::Move(Motion::Left),
                Binding::Custom(Message::Vim(VimAction::Escape)),
            ]))
        } else {
            Some(Binding::Custom(Message::Vim(VimAction::Escape)))
        };
    }

    match vim.mode {
        VimMode::Insert => {
            // In Insert mode, pass through all keys to standard text editor bindings
            None
        }
        VimMode::Normal => handle_normal_mode(vim, key, modified_key, *physical_key, *modifiers),
        VimMode::Visual => handle_visual_mode(vim, key, modified_key, *physical_key, *modifiers, false),
        VimMode::VisualLine => handle_visual_mode(vim, key, modified_key, *physical_key, *modifiers, true),
    }
}

fn handle_normal_mode(
    vim: &mut VimState,
    key: &Key,
    modified_key: &Key,
    physical_key: keyboard::key::Physical,
    modifiers: Modifiers,
) -> Option<Binding<Message>> {
    // Check pending operators first
    if let Some(pending) = vim.pending_operator {
        vim.pending_operator = None;
        match pending {
            VimPendingOperator::G => {
                if matches!(key.to_latin(physical_key), Some('g')) {
                    vim.count_prefix = None;
                    return Some(Binding::Custom(Message::Vim(VimAction::GoToTop)));
                }
            }
            VimPendingOperator::Y => {
                if matches!(key.to_latin(physical_key), Some('y')) {
                    vim.count_prefix = None;
                    return Some(Binding::Custom(Message::Vim(VimAction::YankLine)));
                }
            }
            VimPendingOperator::D => {
                if matches!(key.to_latin(physical_key), Some('d')) {
                    vim.count_prefix = None;
                    return Some(Binding::Custom(Message::Vim(VimAction::DeleteLine)));
                }
            }
        }
    }

    // Ctrl shortcuts
    if modifiers.control() {
        match key.to_latin(physical_key) {
            Some('u') | Some('b') => return repeat_motion(vim, Motion::PageUp),
            Some('d') | Some('f') => return repeat_motion(vim, Motion::PageDown),
            Some('r') => return Some(Binding::Custom(Message::Redo)),
            _ => return None,
        }
    }

    // Pass through system Command/Ctrl shortcuts (e.g. Save, Copy, Paste, etc.)
    if modifiers.command() {
        return None;
    }

    // Digits for count prefix (e.g. 5j, 10w)
    if !modifiers.control() && !modifiers.alt() && !modifiers.command() {
        if let Key::Character(s) = key {
            if let Some(c) = s.chars().next() {
                if ('1'..='9').contains(&c) || (c == '0' && vim.count_prefix.is_some()) {
                    let digit = c.to_digit(10).unwrap() as usize;
                    let new_count = vim.count_prefix.unwrap_or(0) * 10 + digit;
                    vim.count_prefix = Some(new_count);
                    return Some(Binding::Sequence(vec![])); // Consume digit keypress
                }
            }
        }
    }

    // Single-key commands and motions
    match modified_key {
        Key::Named(Named::ArrowLeft) => repeat_motion(vim, Motion::Left),
        Key::Named(Named::ArrowRight) => repeat_motion(vim, Motion::Right),
        Key::Named(Named::ArrowUp) => repeat_motion(vim, Motion::Up),
        Key::Named(Named::ArrowDown) => repeat_motion(vim, Motion::Down),
        Key::Named(Named::Home) => repeat_motion(vim, Motion::Home),
        Key::Named(Named::End) => repeat_motion(vim, Motion::End),
        Key::Named(Named::PageUp) => repeat_motion(vim, Motion::PageUp),
        Key::Named(Named::PageDown) => repeat_motion(vim, Motion::PageDown),
        Key::Named(Named::Enter) => repeat_motion(vim, Motion::Down),
        Key::Named(Named::Backspace) => repeat_motion(vim, Motion::Left),
        Key::Named(Named::Tab) => Some(Binding::Sequence(vec![])),
        _ => {
            if modifiers.control() || modifiers.alt() || modifiers.command() {
                return None;
            }

            match key.to_latin(physical_key) {
                // Movement: Directional
                Some(' ') => repeat_motion(vim, Motion::Right),
                Some('h') => repeat_motion(vim, Motion::Left),
                Some('j') => repeat_motion(vim, Motion::Down),
                Some('k') => repeat_motion(vim, Motion::Up),
                Some('l') => repeat_motion(vim, Motion::Right),

                // Movement: Words
                Some('w') => repeat_motion(vim, Motion::Right.widen()),
                Some('b') => repeat_motion(vim, Motion::Left.widen()),
                Some('e') => repeat_motion(vim, Motion::Right.widen()),

                // Movement: Line boundaries
                Some('0') | Some('^') => {
                    vim.count_prefix = None;
                    Some(Binding::Move(Motion::Home))
                }
                Some('$') => {
                    vim.count_prefix = None;
                    Some(Binding::Move(Motion::End))
                }

                // Movement: Buffer boundaries
                Some('g') => {
                    vim.pending_operator = Some(VimPendingOperator::G);
                    Some(Binding::Sequence(vec![])) // Wait for second 'g'
                }
                Some('G') if modifiers.shift() => {
                    vim.count_prefix = None;
                    Some(Binding::Custom(Message::Vim(VimAction::GoToBottom)))
                }

                // Mode Switching: Insert
                Some('i') => {
                    vim.reset_operator_and_count();
                    vim.mode = VimMode::Insert;
                    Some(Binding::Custom(Message::Vim(VimAction::SetMode(VimMode::Insert))))
                }
                Some('I') if modifiers.shift() => {
                    vim.reset_operator_and_count();
                    vim.mode = VimMode::Insert;
                    Some(Binding::Sequence(vec![
                        Binding::Move(Motion::Home),
                        Binding::Custom(Message::Vim(VimAction::SetMode(VimMode::Insert))),
                    ]))
                }
                Some('a') => {
                    vim.reset_operator_and_count();
                    vim.mode = VimMode::Insert;
                    Some(Binding::Sequence(vec![
                        Binding::Move(Motion::Right),
                        Binding::Custom(Message::Vim(VimAction::SetMode(VimMode::Insert))),
                    ]))
                }
                Some('A') if modifiers.shift() => {
                    vim.reset_operator_and_count();
                    vim.mode = VimMode::Insert;
                    Some(Binding::Sequence(vec![
                        Binding::Move(Motion::End),
                        Binding::Custom(Message::Vim(VimAction::SetMode(VimMode::Insert))),
                    ]))
                }
                Some('o') => {
                    vim.reset_operator_and_count();
                    vim.mode = VimMode::Insert;
                    Some(Binding::Sequence(vec![
                        Binding::Move(Motion::End),
                        Binding::Enter,
                        Binding::Custom(Message::Vim(VimAction::SetMode(VimMode::Insert))),
                    ]))
                }
                Some('O') if modifiers.shift() => {
                    vim.reset_operator_and_count();
                    vim.mode = VimMode::Insert;
                    Some(Binding::Sequence(vec![
                        Binding::Move(Motion::Home),
                        Binding::Enter,
                        Binding::Move(Motion::Up),
                        Binding::Custom(Message::Vim(VimAction::SetMode(VimMode::Insert))),
                    ]))
                }

                // Mode Switching: Visual
                Some('v') => {
                    vim.reset_operator_and_count();
                    vim.mode = VimMode::Visual;
                    Some(Binding::Custom(Message::Vim(VimAction::SetMode(VimMode::Visual))))
                }
                Some('V') if modifiers.shift() => {
                    vim.reset_operator_and_count();
                    vim.mode = VimMode::VisualLine;
                    Some(Binding::Sequence(vec![
                        Binding::SelectLine,
                        Binding::Custom(Message::Vim(VimAction::SetMode(VimMode::VisualLine))),
                    ]))
                }

                // Copy (Yank)
                Some('y') => {
                    vim.pending_operator = Some(VimPendingOperator::Y);
                    Some(Binding::Sequence(vec![])) // Wait for second 'y'
                }
                Some('Y') if modifiers.shift() => {
                    vim.reset_operator_and_count();
                    Some(Binding::Custom(Message::Vim(VimAction::YankLine)))
                }

                // Delete (dd or D)
                Some('d') => {
                    vim.pending_operator = Some(VimPendingOperator::D);
                    Some(Binding::Sequence(vec![])) // Wait for second 'd'
                }
                Some('D') if modifiers.shift() => {
                    vim.reset_operator_and_count();
                    Some(Binding::Sequence(vec![
                        Binding::Select(Motion::End),
                        Binding::Delete,
                    ]))
                }

                // Paste (Put)
                Some('p') => {
                    vim.reset_operator_and_count();
                    Some(Binding::Custom(Message::Vim(VimAction::Paste { before: false })))
                }
                Some('P') if modifiers.shift() => {
                    vim.reset_operator_and_count();
                    Some(Binding::Custom(Message::Vim(VimAction::Paste { before: true })))
                }

                // Single character delete / Undo
                Some('x') => {
                    vim.reset_operator_and_count();
                    Some(Binding::Delete)
                }
                Some('X') if modifiers.shift() => {
                    vim.reset_operator_and_count();
                    Some(Binding::Backspace)
                }
                Some('u') => {
                    vim.reset_operator_and_count();
                    Some(Binding::Custom(Message::Undo))
                }

                // Consume any other character key so NO letters are inserted in Normal mode
                _ => {
                    vim.reset_operator_and_count();
                    Some(Binding::Sequence(vec![]))
                }
            }
        }
    }
}

fn handle_visual_mode(
    vim: &mut VimState,
    key: &Key,
    modified_key: &Key,
    physical_key: keyboard::key::Physical,
    modifiers: Modifiers,
    is_line: bool,
) -> Option<Binding<Message>> {
    // Ctrl shortcuts
    if modifiers.control() {
        match key.to_latin(physical_key) {
            Some('u') | Some('b') => return repeat_select(vim, Motion::PageUp),
            Some('d') | Some('f') => return repeat_select(vim, Motion::PageDown),
            _ => return None,
        }
    }

    if modifiers.command() {
        return None;
    }

    // Digits for count prefix
    if !modifiers.control() && !modifiers.alt() && !modifiers.command() {
        if let Key::Character(s) = key {
            if let Some(c) = s.chars().next() {
                if ('1'..='9').contains(&c) || (c == '0' && vim.count_prefix.is_some()) {
                    let digit = c.to_digit(10).unwrap() as usize;
                    let new_count = vim.count_prefix.unwrap_or(0) * 10 + digit;
                    vim.count_prefix = Some(new_count);
                    return Some(Binding::Sequence(vec![]));
                }
            }
        }
    }

    // Single key motions / selections
    match modified_key {
        Key::Named(Named::ArrowLeft) => repeat_select(vim, Motion::Left),
        Key::Named(Named::ArrowRight) => repeat_select(vim, Motion::Right),
        Key::Named(Named::ArrowUp) => repeat_select(vim, Motion::Up),
        Key::Named(Named::ArrowDown) => repeat_select(vim, Motion::Down),
        Key::Named(Named::Home) => repeat_select(vim, Motion::Home),
        Key::Named(Named::End) => repeat_select(vim, Motion::End),
        Key::Named(Named::PageUp) => repeat_select(vim, Motion::PageUp),
        Key::Named(Named::PageDown) => repeat_select(vim, Motion::PageDown),
        Key::Named(Named::Enter) => repeat_select(vim, Motion::Down),
        Key::Named(Named::Backspace) => repeat_select(vim, Motion::Left),
        Key::Named(Named::Tab) => Some(Binding::Sequence(vec![])),
        _ => {
            if modifiers.control() || modifiers.alt() || modifiers.command() {
                return None;
            }

            match key.to_latin(physical_key) {
                // Movement: Directional
                Some(' ') => repeat_select(vim, Motion::Right),
                Some('h') => repeat_select(vim, Motion::Left),
                Some('j') => repeat_select(vim, Motion::Down),
                Some('k') => repeat_select(vim, Motion::Up),
                Some('l') => repeat_select(vim, Motion::Right),

                // Movement: Words
                Some('w') => repeat_select(vim, Motion::Right.widen()),
                Some('b') => repeat_select(vim, Motion::Left.widen()),
                Some('e') => repeat_select(vim, Motion::Right.widen()),

                // Movement: Line boundaries
                Some('0') | Some('^') => {
                    vim.count_prefix = None;
                    Some(Binding::Select(Motion::Home))
                }
                Some('$') => {
                    vim.count_prefix = None;
                    Some(Binding::Select(Motion::End))
                }

                // Movement: Buffer boundaries
                Some('g') => {
                    vim.count_prefix = None;
                    Some(Binding::Custom(Message::Vim(VimAction::VisualGoToTop)))
                }
                Some('G') if modifiers.shift() => {
                    vim.count_prefix = None;
                    Some(Binding::Custom(Message::Vim(VimAction::VisualGoToBottom)))
                }

                // Copy (Yank) selected text
                Some('y') | Some('Y') => {
                    vim.reset_operator_and_count();
                    vim.mode = VimMode::Normal;
                    vim.last_yank_is_line = is_line;
                    Some(Binding::Sequence(vec![
                        Binding::Copy,
                        Binding::Custom(Message::Vim(VimAction::SetMode(VimMode::Normal))),
                    ]))
                }

                // Delete selected text
                Some('d') | Some('D') | Some('x') | Some('X') => {
                    vim.reset_operator_and_count();
                    vim.mode = VimMode::Normal;
                    Some(Binding::Sequence(vec![
                        Binding::Delete,
                        Binding::Custom(Message::Vim(VimAction::SetMode(VimMode::Normal))),
                    ]))
                }

                // Paste (Replace selection with clipboard text)
                Some('p') | Some('P') => {
                    vim.reset_operator_and_count();
                    vim.mode = VimMode::Normal;
                    Some(Binding::Sequence(vec![
                        Binding::Paste,
                        Binding::Custom(Message::Vim(VimAction::SetMode(VimMode::Normal))),
                    ]))
                }

                // Toggle back to Normal mode on 'v' or 'V'
                Some('v') | Some('V') => {
                    vim.reset_operator_and_count();
                    vim.mode = VimMode::Normal;
                    Some(Binding::Custom(Message::Vim(VimAction::Escape)))
                }

                // Consume any other character key so NO letters are inserted in Visual mode
                _ => {
                    vim.reset_operator_and_count();
                    Some(Binding::Sequence(vec![]))
                }
            }
        }
    }
}

fn repeat_motion(vim: &mut VimState, motion: Motion) -> Option<Binding<Message>> {
    let count = vim.count_prefix.take().unwrap_or(1);
    if count <= 1 {
        Some(Binding::Move(motion))
    } else {
        Some(Binding::Sequence(vec![Binding::Move(motion); count]))
    }
}

fn repeat_select(vim: &mut VimState, motion: Motion) -> Option<Binding<Message>> {
    let count = vim.count_prefix.take().unwrap_or(1);
    if count <= 1 {
        Some(Binding::Select(motion))
    } else {
        Some(Binding::Sequence(vec![Binding::Select(motion); count]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic::iced::keyboard::key::Named;

    fn make_key_press(key: Key, modifiers: Modifiers) -> KeyPress {
        KeyPress {
            key: key.clone(),
            modified_key: key,
            physical_key: keyboard::key::Physical::Unidentified(keyboard::key::NativeCode::Unidentified),
            modifiers,
            text: None,
            status: Status::Focused { is_hovered: true },
        }
    }

    #[test]
    fn test_normal_mode_motions() {
        let mut vim = VimState::default();
        let kp = make_key_press(Key::Character("j".into()), Modifiers::default());
        let res = handle_vim_key_press(&mut vim, &kp);
        assert!(matches!(res, Some(Binding::Move(Motion::Down))));
    }

    #[test]
    fn test_count_prefix_motion() {
        let mut vim = VimState::default();
        let kp5 = make_key_press(Key::Character("5".into()), Modifiers::default());
        let _ = handle_vim_key_press(&mut vim, &kp5);
        assert_eq!(vim.count_prefix, Some(5));

        let kpj = make_key_press(Key::Character("j".into()), Modifiers::default());
        let res = handle_vim_key_press(&mut vim, &kpj);
        if let Some(Binding::Sequence(seq)) = res {
            assert_eq!(seq.len(), 5);
            assert!(matches!(seq[0], Binding::Move(Motion::Down)));
        } else {
            panic!("Expected sequence of 5 moves");
        }
        assert_eq!(vim.count_prefix, None);
    }

    #[test]
    fn test_mode_transitions() {
        let mut vim = VimState::default();
        let kpi = make_key_press(Key::Character("i".into()), Modifiers::default());
        let _ = handle_vim_key_press(&mut vim, &kpi);
        assert_eq!(vim.mode, VimMode::Insert);

        let kpesc = make_key_press(Key::Named(Named::Escape), Modifiers::default());
        let _ = handle_vim_key_press(&mut vim, &kpesc);
        assert_eq!(vim.mode, VimMode::Normal);
    }

    #[test]
    fn test_visual_mode_yank() {
        let mut vim = VimState::default();
        vim.mode = VimMode::Visual;
        let kpy = make_key_press(Key::Character("y".into()), Modifiers::default());
        let res = handle_vim_key_press(&mut vim, &kpy);
        assert_eq!(vim.mode, VimMode::Normal);
        assert!(matches!(res, Some(Binding::Sequence(_))));
    }

    #[test]
    fn test_normal_mode_dd_deletes_line() {
        let mut vim = VimState::default();
        let kpd = make_key_press(Key::Character("d".into()), Modifiers::default());
        let res1 = handle_vim_key_press(&mut vim, &kpd);
        assert!(matches!(res1, Some(Binding::Sequence(seq)) if seq.is_empty()));
        assert_eq!(vim.pending_operator, Some(VimPendingOperator::D));

        let res2 = handle_vim_key_press(&mut vim, &kpd);
        assert!(matches!(res2, Some(Binding::Custom(Message::Vim(VimAction::DeleteLine)))));
        assert_eq!(vim.pending_operator, None);
    }

    #[test]
    fn test_normal_mode_unhandled_letters_consumed() {
        let mut vim = VimState::default();
        let kpz = make_key_press(Key::Character("z".into()), Modifiers::default());
        let res = handle_vim_key_press(&mut vim, &kpz);
        // Unhandled letters must return an empty sequence to consume the key without inserting text
        assert!(matches!(res, Some(Binding::Sequence(seq)) if seq.is_empty()));
    }

    #[test]
    fn test_visual_mode_delete() {
        let mut vim = VimState::default();
        vim.mode = VimMode::Visual;
        let kpd = make_key_press(Key::Character("d".into()), Modifiers::default());
        let res = handle_vim_key_press(&mut vim, &kpd);
        assert_eq!(vim.mode, VimMode::Normal);
        assert!(matches!(res, Some(Binding::Sequence(_))));
    }
}
