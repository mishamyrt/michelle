//! System SF Symbols addressed directly by their AppKit names. The SVG
//! envelope lets GPUI keep its monochrome tinting and spinner transformations;
//! its image is generated in memory from AppKit, never shipped in the bundle.

use std::collections::HashMap;

use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use objc2::{AnyThread, rc::autoreleasepool};
use objc2_app_kit::{
    NSBitmapFormat, NSBitmapImageFileType, NSBitmapImageRep, NSCompositingOperation,
    NSDeviceRGBColorSpace, NSFontWeightRegular, NSGraphicsContext, NSImage,
    NSImageSymbolConfiguration,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};

/// Preloaded before opening a window so drawing never calls AppKit.
pub(super) const SYMBOLS: &[&str] = &[
    "app.badge",
    "arrow.clockwise",
    "arrow.down",
    "arrow.left",
    "arrow.left.arrow.right",
    "arrow.right",
    "arrow.triangle.2.circlepath",
    "arrow.triangle.branch",
    "arrow.turn.down.right",
    "arrow.up",
    "arrow.up.left.and.arrow.down.right",
    "arrow.up.right",
    "asterisk",
    "backward.end",
    "bolt",
    "brain",
    "chart.bar",
    "checkmark",
    "checkmark.seal",
    "chevron.down",
    "chevron.right",
    "chevron.up",
    "chevron.up.chevron.down",
    "circle.lefthalf.filled",
    "command",
    "cpu",
    "cursorarrow.rays",
    "doc",
    "doc.badge.arrow.up",
    "doc.on.doc",
    "doc.richtext",
    "doc.text",
    "doc.zipper",
    "ellipsis",
    "exclamationmark.triangle",
    "externaldrive",
    "eye",
    "eye.slash",
    "film",
    "folder",
    "folder.badge.plus",
    "function",
    "gearshape",
    "globe",
    "hourglass",
    "icloud.and.arrow.up",
    "info.circle",
    "laptopcomputer",
    "list.bullet",
    "lock",
    "lock.open",
    "macwindow.on.rectangle",
    "magnifyingglass",
    "minus",
    "pencil",
    "photo",
    "plus",
    "point.topleft.down.to.point.bottomright.curvepath",
    "scope",
    "server.rack",
    "shippingbox",
    "sidebar.left",
    "sidebar.right",
    "sparkles",
    "square.and.pencil",
    "star",
    "star.fill",
    "stop",
    "stop.fill",
    "terminal",
    "text.badge.plus",
    "textformat",
    "textformat.abc",
    "trash",
    "waveform",
    "wrench.and.screwdriver",
    "xmark",
];

pub(super) fn load() -> Result<HashMap<&'static str, Vec<u8>>> {
    // This entire batch runs on the background executor before the first window.
    // The result is immutable, so there are no render-time probes or locks.
    autoreleasepool(|_| {
        let mut icons = HashMap::with_capacity(SYMBOLS.len());
        for &symbol in SYMBOLS {
            let bytes = render(symbol)
                .or_else(|| {
                    eprintln!("SF Symbol {symbol:?} unavailable; using questionmark.circle");
                    render("questionmark.circle")
                })
                .with_context(|| format!("failed to render SF Symbol {symbol}"))?;
            icons.insert(symbol, bytes);
        }
        Ok(icons)
    })
}

fn render(symbol: &str) -> Option<Vec<u8>> {
    let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(
        &NSString::from_str(symbol),
        None,
    )?;
    let configuration =
        NSImageSymbolConfiguration::configurationWithPointSize_weight(16.0, unsafe {
            NSFontWeightRegular
        });
    let image = image.imageWithSymbolConfiguration(&configuration)?;
    let natural = image.size();
    // A 128px source covers the largest chrome icons at Retina scale. Preserve
    // the symbol's proportions and the padding used by our existing icon slots.
    let scale = 112.0 / natural.width.max(natural.height);
    let raster_size = NSSize::new(natural.width * scale, natural.height * scale);
    // AppKit's CGImage extraction can choose a Retina / high-depth rep. Draw
    // into an explicit 8-bit buffer instead, bounding both decode cost and RAM.
    let bitmap = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bitmapFormat_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(), std::ptr::null_mut(), 128, 128, 8, 4, true,
            false, NSDeviceRGBColorSpace, NSBitmapFormat::empty(), 128 * 4, 32,
        )
    }?;
    let pixels = bitmap.bitmapData();
    if pixels.is_null() {
        return None;
    }
    // The representation owns exactly 128 rows at the requested row stride.
    unsafe { std::ptr::write_bytes(pixels, 0, bitmap.bytesPerRow() as usize * 128) };
    let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&bitmap)?;
    let rect = NSRect::new(
        NSPoint::new(
            (128.0 - raster_size.width) / 2.0,
            (128.0 - raster_size.height) / 2.0,
        ),
        raster_size,
    );
    NSGraphicsContext::saveGraphicsState_class();
    NSGraphicsContext::setCurrentContext(Some(&context));
    image.drawInRect_fromRect_operation_fraction(
        rect,
        NSRect::ZERO,
        NSCompositingOperation::Copy,
        1.0,
    );
    NSGraphicsContext::restoreGraphicsState_class();
    let png = unsafe {
        bitmap.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }?;
    let data = STANDARD.encode(unsafe { png.as_bytes_unchecked() });
    Some(format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128"><image width="128" height="128" href="data:image/png;base64,{data}"/></svg>"#).into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::Arc, time::Instant};

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
                let mut names = std::collections::HashSet::new();
                for &symbol in SYMBOLS {
                    assert!(names.insert(symbol), "duplicate SF Symbol: {symbol}");
                    let svg =
                        render(symbol).unwrap_or_else(|| panic!("unavailable SF Symbol: {symbol}"));
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
                }
                eprintln!(
                    "{} SF Symbols: {} KiB, native extraction and GPUI decoding in {:?}; slowest first decode {:?}",
                    SYMBOLS.len(),
                    bytes / 1024,
                    started.elapsed(),
                    slowest_decode,
                );
            })
        })
        .join()
        .unwrap();
    }
}
