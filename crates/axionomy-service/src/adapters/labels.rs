//! What every adapter uses to name and write its problem's values; each
//! problem's own labels and formats live in its adapter.

use axionomy_view::QuantityFormat;
use std::fmt::Debug;

pub(crate) trait StudioLabel {
    fn studio_label(&self) -> String;
}

/// How Studio writes quantities of an asset: a count unless the problem
/// stores the asset scaled.
pub(crate) trait StudioFormat {
    fn studio_format(&self) -> QuantityFormat {
        QuantityFormat::COUNT
    }
}

macro_rules! counted {
    ($($type:path),+ $(,)?) => {$(
        impl StudioFormat for $type {}
    )+};
}

pub(super) fn words(value: &impl Debug) -> String {
    let debug = format!("{value:?}");
    let mut output = String::with_capacity(debug.len() + 4);
    let mut previous_lowercase = false;
    for character in debug.chars() {
        if matches!(character, '(' | ')' | '{' | '}' | '[' | ']') {
            output.push(' ');
            previous_lowercase = false;
        } else if character == ',' {
            output.push_str(", ");
            previous_lowercase = false;
        } else if character == '_' {
            output.push(' ');
            previous_lowercase = false;
        } else {
            if character.is_ascii_uppercase() && previous_lowercase {
                output.push(' ');
            }
            output.push(character);
            previous_lowercase = character.is_ascii_lowercase() || character.is_ascii_digit();
        }
    }
    output.split_whitespace().collect::<Vec<_>>().join(" ")
}

macro_rules! debug_labels {
    ($($type:path),+ $(,)?) => {$(
        impl StudioLabel for $type {
            fn studio_label(&self) -> String { words(self) }
        }
    )+};
}

pub(super) use counted;
pub(super) use debug_labels;
