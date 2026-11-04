//! A one-line text entry with a movable cursor, shared by the name fields.

use crate::GameKey;

/// Text plus a cursor counted in chars, so Left/Right step over a whole
/// character whatever its UTF-8 width.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextField {
    text: String,
    cursor: usize,
}

impl TextField {
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Replaces the text and puts the cursor after it.
    pub fn set(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.cursor = self.text.chars().count();
    }

    /// The text either side of the cursor.
    pub fn split_at_cursor(&self) -> (&str, &str) {
        self.text.split_at(self.byte_at(self.cursor))
    }

    /// Applies an editing key; `false` means the key was not one of them.
    /// A printable char past `max_chars` is swallowed, not passed on.
    pub fn edit(&mut self, key: GameKey, max_chars: usize) -> bool {
        match key {
            GameKey::Left => self.cursor = self.cursor.saturating_sub(1),
            GameKey::Right => self.cursor = (self.cursor + 1).min(self.text.chars().count()),
            GameKey::Backspace => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.text.remove(self.byte_at(self.cursor));
                }
            }
            GameKey::Char(c) if !c.is_control() => {
                if self.text.chars().count() < max_chars {
                    self.text.insert(self.byte_at(self.cursor), c);
                    self.cursor += 1;
                }
            }
            _ => return false,
        }
        true
    }

    fn byte_at(&self, chars: usize) -> usize {
        self.text
            .char_indices()
            .nth(chars)
            .map_or(self.text.len(), |(i, _)| i)
    }
}

impl From<&str> for TextField {
    fn from(text: &str) -> Self {
        let mut field = Self::default();
        field.set(text);
        field
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(field: &mut TextField, keys: &[GameKey]) {
        for &key in keys {
            field.edit(key, 8);
        }
    }

    #[test]
    fn typing_after_moving_left_inserts_at_the_cursor() {
        let mut field = TextField::from("Mend");
        press(
            &mut field,
            &[GameKey::Left, GameKey::Left, GameKey::Char('x')],
        );
        assert_eq!(field.as_str(), "Mexnd");
        assert_eq!(field.split_at_cursor(), ("Mex", "nd"));
    }

    #[test]
    fn backspace_deletes_the_char_before_the_cursor() {
        let mut field = TextField::from("Mend");
        press(&mut field, &[GameKey::Left, GameKey::Backspace]);
        assert_eq!(field.as_str(), "Med");
        assert_eq!(field.split_at_cursor(), ("Me", "d"));
    }

    #[test]
    fn the_cursor_stops_at_both_ends() {
        let mut field = TextField::from("ab");
        press(&mut field, &[GameKey::Right, GameKey::Right]);
        assert_eq!(field.split_at_cursor(), ("ab", ""));
        press(&mut field, &[GameKey::Left; 4]);
        assert_eq!(field.split_at_cursor(), ("", "ab"));
        press(&mut field, &[GameKey::Backspace]);
        assert_eq!(field.as_str(), "ab");
    }

    #[test]
    fn the_cursor_steps_over_a_multibyte_char() {
        let mut field = TextField::from("aé");
        press(&mut field, &[GameKey::Left, GameKey::Char('z')]);
        assert_eq!(field.as_str(), "azé");
    }

    #[test]
    fn a_full_field_swallows_chars_but_still_moves() {
        let mut field = TextField::from("12345678");
        assert!(field.edit(GameKey::Char('9'), 8));
        assert_eq!(field.as_str(), "12345678");
        assert!(field.edit(GameKey::Left, 8));
        assert!(!field.edit(GameKey::Enter, 8));
    }
}
