use super::macos::render;
use super::*;
use crate::ui::sp;
use base64::{Engine, engine::general_purpose::STANDARD};
use gpui::{Styled, px, rgb};
use objc2::rc::autoreleasepool;
use objc2_app_kit::{NSFontWeightSemibold, NSImage, NSImageSymbolConfiguration};
use objc2_foundation::NSString;
use std::{sync::Arc, time::Instant};

#[test]
fn disclosure_chevrons_match_native_nine_point_symbols() {
    std::thread::spawn(|| {
        autoreleasepool(|_| {
            load().unwrap();
            for name in ["chevron.right", "chevron.down"] {
                let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(
                    &NSString::from_str(name),
                    None,
                )
                .unwrap();
                let configuration =
                    NSImageSymbolConfiguration::configurationWithPointSize_weight(9.0, unsafe {
                        NSFontWeightSemibold
                    });
                let native = image
                    .imageWithSymbolConfiguration(&configuration)
                    .unwrap()
                    .size();
                let (_, width, height) = font_icon(name, 9.0, SfSymbolWeight::Semibold).unwrap();
                assert_eq!(
                    (width, height),
                    (native.width as f32, native.height as f32),
                    "{name} must use its native 9pt geometry",
                );
            }
        });
    })
    .join()
    .unwrap();
}

#[test]
fn every_system_icon_renders_a_nonempty_mask() {
    // Exercise AppKit on a worker, just as startup does, and decode the SVG
    // envelopes through GPUI. Catch absent names, opaque backgrounds and
    // empty PNGs before they can erase a control's meaning.
    std::thread::spawn(|| {
            autoreleasepool(|_| {
                let renderer = gpui::SvgRenderer::new(Arc::new(crate::assets::Assets::default()));
                let started = Instant::now();
                let mut bytes = 0;
                let mut slowest_decode = std::time::Duration::ZERO;
                let mut native_duration = std::time::Duration::ZERO;
                let mut names = std::collections::HashSet::new();
                for &symbol in SYMBOLS {
                    assert!(names.insert(symbol), "duplicate SF Symbol: {symbol}");
                    let native_started = Instant::now();
                    let symbol_image = render(symbol, SfSymbolWeight::Regular)
                        .unwrap_or_else(|| panic!("unavailable SF Symbol: {symbol}"));
                    native_duration += native_started.elapsed();
                    let svg = symbol_image.square_svg;
                    let envelope = std::str::from_utf8(&svg).unwrap();
                    let encoded = envelope
                        .split("base64,")
                        .nth(1)
                        .unwrap()
                        .split('"')
                        .next()
                        .unwrap();
                    let png = STANDARD.decode(encoded).unwrap();
                    let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
                    let height = u32::from_be_bytes(png[20..24].try_into().unwrap());
                    assert!(
                        width == 128 && height == 128,
                        "unexpected raster size: {symbol} ({width}×{height})"
                    );
                    bytes += svg.len();
                    let decode_started = Instant::now();
                    let image = renderer.render_single_frame(&svg, 0.25).unwrap();
                    slowest_decode = slowest_decode.max(decode_started.elapsed());
                    let pixels = image.as_bytes(0).unwrap();
                    assert!(
                        pixels.chunks_exact(4).any(|pixel| pixel[3] > 0),
                        "empty symbol: {symbol}"
                    );
                    assert!(
                        pixels.chunks_exact(4).any(|pixel| pixel[3] == 0),
                        "opaque background: {symbol}"
                    );
                    let natural = renderer
                        .render_single_frame(&symbol_image.natural_svg, 1.0)
                        .unwrap();
                    let natural_size = natural.size(0);
                    assert_eq!(
                        (u32::from(natural_size.width), u32::from(natural_size.height)),
                        (
                            symbol_image.natural_size.width as u32 * 2,
                            symbol_image.natural_size.height as u32 * 2,
                        ),
                        "native aspect ratio lost: {symbol}"
                    );
                    assert!(
                        natural.as_bytes(0).unwrap().chunks_exact(4).any(|pixel| pixel[3] > 0),
                        "empty natural symbol: {symbol}"
                    );
                }
                eprintln!(
                    "{} SF Symbols: {} KiB, native extraction {:?}, total extraction and GPUI decoding {:?}; slowest first decode {:?}",
                    SYMBOLS.len(),
                    bytes / 1024,
                    native_duration,
                    started.elapsed(),
                    slowest_decode,
                );
            })
        })
        .join()
        .unwrap();
}

#[test]
fn font_sized_symbols_keep_native_proportions() {
    use gpui::AssetSource;

    let assets = crate::assets::Assets::default();
    let started = std::time::Instant::now();
    assets.prepare_system_icons().unwrap();
    eprintln!("SF Symbol weight table prepared in {:?}", started.elapsed());
    let (_, width, height) = font_icon("sidebar.left", 15.0, SfSymbolWeight::Regular).unwrap();
    assert!(width > height, "a wide symbol must not become square");
    let (_, double_width, double_height) =
        font_icon("sidebar.left", 30.0, SfSymbolWeight::Regular).unwrap();
    assert_eq!((double_width, double_height), (width * 2.0, height * 2.0));
    let (_, dots_width, dots_height) =
        font_icon("ellipsis", 15.0, SfSymbolWeight::Regular).unwrap();
    assert!(dots_height < height / 2.0 && dots_width > dots_height * 2.0);

    let mut image = sf_icon("sidebar.left", 15.0, rgb(0xffffff).into()).into_svg(gpui::black());
    assert_eq!(image.style().size.width, Some(sp(width).into()));
    assert_eq!(image.style().size.height, Some(sp(height).into()));

    let renderer = gpui::SvgRenderer::new(std::sync::Arc::new(assets.clone()));
    let mut masks = Vec::new();
    for weight in SfSymbolWeight::ALL {
        let (path, width, height) = font_icon("folder", 17.0, weight).unwrap();
        let (_, double_width, double_height) = font_icon("folder", 34.0, weight).unwrap();
        assert_eq!((double_width, double_height), (width * 2.0, height * 2.0));
        let bytes = assets.load(&path).unwrap().unwrap();
        let rendered = renderer.render_single_frame(&bytes, 1.0).unwrap();
        let alpha = rendered
            .as_bytes(0)
            .unwrap()
            .chunks_exact(4)
            .map(|pixel| pixel[3])
            .collect::<Vec<_>>();
        assert!(
            alpha.iter().any(|&pixel| pixel > 0),
            "empty {weight:?} icon"
        );
        assert!(
            !masks.contains(&alpha),
            "{weight:?} reused another weight's mask"
        );
        masks.push(alpha);

        let mut image = sf_icon("folder", 17.0, rgb(0xffffff).into())
            .weight(weight)
            .into_svg(gpui::black());
        assert_eq!(image.style().size.width, Some(sp(width).into()));
        assert_eq!(image.style().size.height, Some(sp(height).into()));
        let mut sized = sf_icon("folder", 17.0, rgb(0xffffff).into())
            .w(px(23.0))
            .weight(weight)
            .h(px(25.0))
            .into_svg(gpui::black());
        assert_eq!(sized.style().size.width, Some(px(23.0).into()));
        assert_eq!(sized.style().size.height, Some(px(25.0).into()));
    }
    assert!(assets.load("folder").unwrap().is_some());
    assert!(assets.load("sf-symbols/heavy/folder").unwrap().is_none());
}

#[test]
fn symbols_supply_the_foreground_required_by_the_svg_painter() {
    for foreground in [gpui::black(), gpui::white()] {
        let mut check = SfSymbol::new("checkmark").into_svg(foreground);
        assert_eq!(check.style().text.color, Some(foreground));

        let explicit = rgb(0xff0000).into();
        let mut colored = SfSymbol::new("checkmark")
            .color(explicit)
            .into_svg(foreground);
        assert_eq!(colored.style().text.color, Some(explicit));
    }
}
