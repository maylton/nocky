//! Material Expressive button foundation.
//!
//! The contract is introduced incrementally so each layer is validated before
//! existing widgets begin to consume it.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaterialButtonVariant {
    Filled,
    FilledTonal,
    Elevated,
    Outlined,
    Text,
}

impl MaterialButtonVariant {
    pub const fn css_class(self) -> &'static str {
        match self {
            Self::Filled => "material-button-filled",
            Self::FilledTonal => "material-button-filled-tonal",
            Self::Elevated => "material-button-elevated",
            Self::Outlined => "material-button-outlined",
            Self::Text => "material-button-text",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaterialButtonSize {
    Compact,
    Standard,
    Large,
}

impl MaterialButtonSize {
    pub const fn css_class(self) -> &'static str {
        match self {
            Self::Compact => "material-button-compact",
            Self::Standard => "material-button-standard",
            Self::Large => "material-button-large",
        }
    }
}
