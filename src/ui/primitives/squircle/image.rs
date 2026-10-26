//! Preserve GPUI's image layout, loading and input; cover only its corner
//! pixels against a uniform parent surface with the shared squircle geometry.

use std::{cell::Cell, rc::Rc, sync::Arc};

use gpui::{
    App, Asset, AssetLogger, Bounds, Corners, DevicePixels, Element, ElementId, GlobalElementId,
    Hitbox, Hsla, Image, ImageCacheError, ImageSource, Img, ImgResourceLoader, InspectorElementId,
    InteractiveElement, Interactivity, IntoElement, LayoutId, ObjectFit, Pixels, RenderImage, Size,
    StatefulInteractiveElement, StyleRefinement, Styled, StyledImage, Window, img,
};

pub struct SquircleImage {
    image: Img,
    radius: Pixels,
    backdrop: Hsla,
    image_size: Rc<Cell<Option<Size<DevicePixels>>>>,
    object_fit: ObjectFit,
}

impl IntoElement for SquircleImage {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

/// The backdrop must be the opaque, uniform surface behind the image.
pub fn squircle_image(
    source: impl Into<ImageSource>,
    radius: Pixels,
    backdrop: Hsla,
) -> SquircleImage {
    let source = source.into();
    let image_size = Rc::new(Cell::new(None));
    let loaded_size = image_size.clone();
    let image = img(move |window: &mut Window, cx: &mut App| {
        let result = match &source {
            ImageSource::Render(data) => Some(Ok(data.clone())),
            ImageSource::Resource(resource) => window.use_asset::<ImgResourceLoader>(resource, cx),
            ImageSource::Image(image) => window.use_asset::<AssetLogger<ImageDecoder>>(image, cx),
            ImageSource::Custom(load) => load(window, cx),
        };
        loaded_size.set(
            result
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                .filter(|data| data.frame_count() > 0)
                .map(|data| data.size(0)),
        );
        result
    });
    SquircleImage {
        image,
        radius,
        backdrop,
        image_size,
        object_fit: ObjectFit::Contain,
    }
}

// Use GPUI's asynchronous asset lifecycle; decoding must never run during paint.
struct ImageDecoder;
impl Asset for ImageDecoder {
    type Source = Arc<Image>;
    type Output = Result<Arc<RenderImage>, ImageCacheError>;

    fn load(
        source: Self::Source,
        cx: &mut App,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let renderer = cx.svg_renderer();
        async move { source.to_image_data(renderer).map_err(Into::into) }
    }
}

impl SquircleImage {
    pub fn object_fit(mut self, object_fit: ObjectFit) -> Self {
        self.object_fit = match object_fit {
            ObjectFit::Fill => ObjectFit::Fill,
            ObjectFit::Contain => ObjectFit::Contain,
            ObjectFit::Cover => ObjectFit::Cover,
            ObjectFit::ScaleDown => ObjectFit::ScaleDown,
            ObjectFit::None => ObjectFit::None,
        };
        self.image = self.image.object_fit(object_fit);
        self
    }
}

impl Styled for SquircleImage {
    fn style(&mut self) -> &mut StyleRefinement {
        self.image.style()
    }
}

impl InteractiveElement for SquircleImage {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.image.interactivity()
    }
}

impl StatefulInteractiveElement for SquircleImage {}

impl Element for SquircleImage {
    type RequestLayoutState = <Img as Element>::RequestLayoutState;
    type PrepaintState = Option<Hitbox>;

    fn id(&self) -> Option<ElementId> {
        Element::id(&self.image)
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        self.image.source_location()
    }

    fn a11y_role(&self) -> Option<gpui::accesskit::Role> {
        self.image.a11y_role()
    }

    fn write_a11y_info(&self, node: &mut gpui::accesskit::Node) {
        self.image.write_a11y_info(node);
    }

    fn a11y_synthetic_children(
        &mut self,
        prepaint: &mut Self::PrepaintState,
        builder: &mut gpui::A11ySubtreeBuilder,
    ) {
        Element::a11y_synthetic_children(&mut self.image, prepaint, builder);
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.image.request_layout(id, inspector, window, cx)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.image
            .prepaint(id, inspector, bounds, layout, window, cx)
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.image
            .paint(id, inspector, bounds, layout, hitbox, window, cx);
        if let Some(image_size) = self.image_size.get() {
            let visible_bounds = bounds.intersect(&self.object_fit.get_bounds(bounds, image_size));
            super::element::paint_corner_mask(
                window,
                visible_bounds,
                Corners::all(self.radius),
                self.backdrop,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{
        Context, FocusHandle, Modifiers, MouseButton, ParentElement, Render, TestAppContext, div,
        point, px, rgb,
    };

    struct Harness {
        data: Arc<RenderImage>,
        focus: FocusHandle,
        clicks: Rc<Cell<usize>>,
        loaded_size: Rc<Cell<Option<Size<DevicePixels>>>>,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let image = squircle_image(self.data.clone(), px(9.0), rgb(0x123456).into());
            let clicks = self.clicks.clone();
            self.loaded_size = image.image_size.clone();
            div().p(px(10.0)).child(
                image
                    .id("masked-image")
                    .w(px(80.0))
                    .bg(rgb(0x654321))
                    .track_focus(&self.focus)
                    .tab_index(0)
                    .on_click(move |_, _, _| clicks.set(clicks.get() + 1)),
            )
        }
    }

    #[gpui::test]
    fn fitted_image_keeps_intrinsic_layout_identity_and_input(cx: &mut TestAppContext) {
        let focus = cx.update(|cx| cx.focus_handle());
        let data = cx.update(|cx| {
            Image::from_bytes(gpui::ImageFormat::Svg,
                br##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50"><rect width="100" height="50" fill="#fff"/></svg>"##.to_vec())
                .to_image_data(cx.svg_renderer()).unwrap()
        });
        let image_size = data.size(0);
        let clicks = Rc::new(Cell::new(0));
        let (view, cx) = cx.add_window_view(|_, _| Harness {
            data,
            focus: focus.clone(),
            clicks: clicks.clone(),
            loaded_size: Rc::default(),
        });
        view.read_with(cx, |view, _| {
            assert_eq!(view.loaded_size.get().unwrap(), image_size)
        });
        cx.update(|window, _| {
            let quad = window
                .painted_quads()
                .into_iter()
                .find(|quad| quad.background.as_solid() == Some(rgb(0x654321).into()))
                .unwrap();
            let scale = window.scale_factor();
            assert_eq!(quad.bounds.origin.x.0, 10.0 * scale);
            assert_eq!(quad.bounds.origin.y.0, 10.0 * scale);
            assert_eq!(quad.bounds.size.width.0, 80.0 * scale);
            assert_eq!(quad.bounds.size.height.0, 40.0 * scale);
        });
        cx.simulate_mouse_move(point(px(30.0), px(30.0)), None, Modifiers::none());
        cx.simulate_mouse_down(
            point(px(30.0), px(30.0)),
            MouseButton::Left,
            Modifiers::none(),
        );
        cx.simulate_mouse_up(
            point(px(30.0), px(30.0)),
            MouseButton::Left,
            Modifiers::none(),
        );
        assert_eq!(clicks.get(), 1);
        cx.simulate_keystrokes("tab");
        cx.update(|window, _| assert!(focus.is_focused(window)));
    }
}
