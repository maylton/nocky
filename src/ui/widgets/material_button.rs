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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MaterialButtonSemantic {
    #[default]
    Standard,
    Destructive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaterialButtonSpec {
    pub variant: MaterialButtonVariant,
    pub size: MaterialButtonSize,
    pub semantic: MaterialButtonSemantic,
}

impl MaterialButtonSpec {
    pub const fn new(variant: MaterialButtonVariant, size: MaterialButtonSize) -> Self {
        Self {
            variant,
            size,
            semantic: MaterialButtonSemantic::Standard,
        }
    }

    pub const fn with_semantic(mut self, semantic: MaterialButtonSemantic) -> Self {
        self.semantic = semantic;
        self
    }

    pub fn css_classes(self) -> Vec<&'static str> {
        let mut classes = vec![self.variant.css_class(), self.size.css_class()];
        if self.semantic == MaterialButtonSemantic::Destructive {
            classes.push("material-button-destructive");
        }
        classes
    }
}
