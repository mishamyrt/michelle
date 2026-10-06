//! SF Symbols with native proportions, prepared before the first window.

mod cache;
mod catalog;
mod element;
mod macos;
#[cfg(test)]
mod tests;
mod weight;

pub(crate) use cache::{font_icon, load};
pub(crate) use catalog::SYMBOLS;
pub use element::{SfIcon, SfSymbol, sf_icon};
pub(crate) use macos::SystemSymbol;
pub(crate) use weight::parse_font_icon_path;
pub use weight::{SfSymbolWeight, SymbolWeight};
