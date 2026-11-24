//! A [`Text`] as the layout takes it: the string drawn and the spans that
//! say how each part of it is set.

use std::borrow::Cow;

use voltra_assets::fonts::{RunStyle, Span};
use voltra_assets::{Font, Fonts, Handle};

use super::Text;

/// What `text` lays out as at `px` raster pixels per em in `font`: its
/// content as one span.
pub fn spans<'a>(
    text: &'a Text,
    _fonts: &Fonts,
    font: Handle<Font>,
    px: u32,
) -> (Cow<'a, str>, Vec<Span>) {
    let content = Cow::Borrowed(text.content.as_str());
    let spans = vec![Span::whole(&content, RunStyle::plain(font, px))];
    (content, spans)
}
