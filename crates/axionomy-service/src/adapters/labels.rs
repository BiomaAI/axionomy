//! What every adapter uses to name and write its problem's values; each
//! problem's own labels and formats live in its adapter.

use axionomy_view::QuantityFormat;

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

/// A value's `Debug` output as words, as every view labels it.
pub(super) use axionomy_view::words;

macro_rules! debug_labels {
    ($($type:path),+ $(,)?) => {$(
        impl StudioLabel for $type {
            fn studio_label(&self) -> String { words(self) }
        }
    )+};
}

pub(super) use counted;
pub(super) use debug_labels;
