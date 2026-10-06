use super::macos::{reference_point_size, render};
use super::{SYMBOLS, SfSymbolWeight, SystemSymbol};
use anyhow::{Context, Result};
use gpui::SharedString;
use objc2::rc::autoreleasepool;
use std::{collections::HashMap, sync::OnceLock};

// Published by the startup worker; a frame only reads this immutable table.
type FontIcon = (SharedString, f32, f32);
static FONT_ICONS: OnceLock<HashMap<&'static str, [FontIcon; 4]>> = OnceLock::new();

pub(crate) fn font_icon(name: &str, font_size: f32, weight: SfSymbolWeight) -> Option<FontIcon> {
    let (path, width, height) = &FONT_ICONS.get()?.get(name)?[weight as usize];
    Some((path.clone(), width * font_size, height * font_size))
}

pub(crate) fn load() -> Result<HashMap<&'static str, [SystemSymbol; 4]>> {
    // This entire batch runs on the background executor before the first window.
    // The result is immutable, so there are no render-time probes or locks.
    autoreleasepool(|_| {
        let mut icons = HashMap::with_capacity(SYMBOLS.len());
        for &symbol in SYMBOLS {
            let [regular, medium, semibold, bold] = SfSymbolWeight::ALL.map(|weight| {
                render(symbol, weight)
                    .or_else(|| {
                        eprintln!("SF Symbol {symbol:?} ({weight:?}) unavailable; using questionmark.circle");
                        render("questionmark.circle", weight)
                    })
                    .with_context(|| format!("failed to render SF Symbol {symbol} ({weight:?})"))
            });
            icons.insert(symbol, [regular?, medium?, semibold?, bold?]);
        }
        let _ = FONT_ICONS.set(
            icons
                .iter()
                .map(|(&name, images)| {
                    (
                        name,
                        std::array::from_fn(|index| {
                            let image = &images[index];
                            (
                                SfSymbolWeight::ALL[index].path(name),
                                (image.natural_size.width / reference_point_size(name)) as f32,
                                (image.natural_size.height / reference_point_size(name)) as f32,
                            )
                        }),
                    )
                })
                .collect(),
        );
        Ok(icons)
    })
}
