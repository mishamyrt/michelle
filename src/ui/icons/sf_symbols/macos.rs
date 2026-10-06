use base64::{Engine, engine::general_purpose::STANDARD};
use objc2::AnyThread;
use objc2_app_kit::{
    NSBitmapFormat, NSBitmapImageFileType, NSBitmapImageRep, NSCompositingOperation,
    NSDeviceRGBColorSpace, NSFontWeightBold, NSFontWeightMedium, NSFontWeightRegular,
    NSFontWeightSemibold, NSGraphicsContext, NSImage, NSImageSymbolConfiguration,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};

use super::SfSymbolWeight;

const POINT_SIZE: f64 = 16.0;

pub(super) fn reference_point_size(symbol: &str) -> f64 {
    match symbol {
        // AppKit's small chevrons have different optical geometry. Scaling
        // their 16pt variants down makes the two disclosure states uneven.
        "chevron.right" | "chevron.down" => 9.0,
        _ => POINT_SIZE,
    }
}

pub(crate) struct SystemSymbol {
    pub square_svg: Vec<u8>,
    pub natural_svg: Vec<u8>,
    pub(super) natural_size: NSSize,
}

pub(super) fn render(symbol: &str, weight: SfSymbolWeight) -> Option<SystemSymbol> {
    let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(
        &NSString::from_str(symbol),
        None,
    )?;
    let configuration = NSImageSymbolConfiguration::configurationWithPointSize_weight(
        reference_point_size(symbol),
        unsafe {
            match weight {
                SfSymbolWeight::Regular => NSFontWeightRegular,
                SfSymbolWeight::Medium => NSFontWeightMedium,
                SfSymbolWeight::Semibold => NSFontWeightSemibold,
                SfSymbolWeight::Bold => NSFontWeightBold,
            }
        },
    );
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
    let square_svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128"><image width="128" height="128" href="data:image/png;base64,{data}"/></svg>"#).into_bytes();
    // Crop only the square slot's extra padding, retaining AppKit's own image
    // bounds. GPUI rasterizes SVGs by width, so the viewBox and layout must both
    // keep the native aspect ratio (including for short symbols like ellipsis).
    let natural_svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="{} {} {} {}"><image width="128" height="128" href="data:image/png;base64,{data}"/></svg>"#,
        natural.width, natural.height, rect.origin.x, rect.origin.y,
        rect.size.width, rect.size.height,
    ).into_bytes();
    Some(SystemSymbol {
        square_svg,
        natural_svg,
        natural_size: natural,
    })
}
