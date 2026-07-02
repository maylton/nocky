#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum MaterialCarouselStrategy {
    Hero,
    #[default]
    MultiBrowse,
    Uncontained,
}
