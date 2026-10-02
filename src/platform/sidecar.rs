//! Bridge Sidecar's native touches to GPUI's tap, pan, and momentum recognition.

use std::cell::{Cell, RefCell};

use gpui::{
    App, AsyncWindowContext, Pixels, PlatformInput, Point, TouchEvent, TouchId, TouchPhase, Window,
    point, px,
};
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
    sel,
};
use objc2_app_kit::{
    NSEvent, NSGestureRecognizer, NSGestureRecognizerDelegate, NSGestureRecognizerState, NSTouch,
    NSTouchPhase, NSTouchType, NSTouchTypeMask, NSView,
};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSProcessInfo};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

#[derive(Default)]
struct TouchStream {
    next_id: u64,
    active: Option<TouchId>,
    position: Point<Pixels>,
}

impl TouchStream {
    fn event(&mut self, phase: TouchPhase, position: Point<Pixels>) -> Option<TouchEvent> {
        if phase == TouchPhase::Started {
            if self.active.is_some() {
                return None;
            }
            self.next_id += 1;
            self.active = Some(TouchId(self.next_id));
        }
        let id = self.active?;
        self.position = position;
        if matches!(phase, TouchPhase::Ended | TouchPhase::Cancelled) {
            self.active = None;
        }
        Some(TouchEvent {
            id,
            phase,
            position,
            ..Default::default()
        })
    }
}

fn gpui_position(location: NSPoint, height: f64, flipped: bool) -> Point<Pixels> {
    point(
        px(location.x as f32),
        px(if flipped {
            location.y
        } else {
            height - location.y
        } as f32),
    )
}

struct TouchIvars {
    cx: AsyncWindowContext,
    identity: RefCell<Option<Retained<AnyObject>>>,
    stream: RefCell<TouchStream>,
    pending: RefCell<Vec<TouchEvent>>,
    location: Cell<NSPoint>,
}

define_class!(
    // SAFETY: this recognizer forwards only direct touches, updates its state
    // during event processing, and dispatches to GPUI from its action callback.
    #[unsafe(super(NSGestureRecognizer))]
    #[thread_kind = MainThreadOnly]
    #[ivars = TouchIvars]
    struct SidecarTouchRecognizer;

    unsafe impl NSObjectProtocol for SidecarTouchRecognizer {}

    unsafe impl NSGestureRecognizerDelegate for SidecarTouchRecognizer {
        // allowedTouchTypes does not filter mouse events. Keep those on GPUI's
        // existing native mouse/trackpad path.
        #[unsafe(method(gestureRecognizer:shouldAttemptToRecognizeWithEvent:))]
        fn should_attempt(&self, _: &NSGestureRecognizer, _: &NSEvent) -> bool {
            false
        }

        #[unsafe(method(gestureRecognizer:shouldReceiveTouch:))]
        fn should_receive_touch(&self, _: &NSGestureRecognizer, touch: &NSTouch) -> bool {
            touch.r#type() == NSTouchType::Direct
                && self.ivars().identity.borrow().is_none()
        }
    }

    impl SidecarTouchRecognizer {
        #[unsafe(method(touchesBeganWithEvent:))]
        fn touches_began(&self, event: &NSEvent) {
            unsafe { let _: () = msg_send![super(self), touchesBeganWithEvent: event]; }
            self.collect_touches(event, NSTouchPhase::Began, TouchPhase::Started);
        }

        #[unsafe(method(touchesMovedWithEvent:))]
        fn touches_moved(&self, event: &NSEvent) {
            unsafe { let _: () = msg_send![super(self), touchesMovedWithEvent: event]; }
            self.collect_touches(event, NSTouchPhase::Moved, TouchPhase::Moved);
        }

        #[unsafe(method(touchesEndedWithEvent:))]
        fn touches_ended(&self, event: &NSEvent) {
            unsafe { let _: () = msg_send![super(self), touchesEndedWithEvent: event]; }
            self.collect_touches(event, NSTouchPhase::Ended, TouchPhase::Ended);
        }

        #[unsafe(method(touchesCancelledWithEvent:))]
        fn touches_cancelled(&self, event: &NSEvent) {
            unsafe { let _: () = msg_send![super(self), touchesCancelledWithEvent: event]; }
            self.collect_touches(event, NSTouchPhase::Cancelled, TouchPhase::Cancelled);
        }

        #[unsafe(method(locationInView:))]
        fn location_in_view(&self, view: Option<&NSView>) -> NSPoint {
            self.view().map_or(NSPoint::new(0.0, 0.0), |source| {
                source.convertPoint_toView(self.ivars().location.get(), view)
            })
        }

        #[unsafe(method(deliverTouches:))]
        fn deliver_touches(&self, _: &NSGestureRecognizer) {
            let events = self.ivars().pending.take();
            self.ivars().cx.clone().update(move |window, cx| {
                for event in events {
                    window.dispatch_event(PlatformInput::Touch(event), cx);
                }
            }).ok();
        }
    }
);

impl SidecarTouchRecognizer {
    fn new(mtm: MainThreadMarker, cx: AsyncWindowContext) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(TouchIvars {
            cx,
            identity: RefCell::new(None),
            stream: RefCell::new(TouchStream::default()),
            pending: RefCell::new(Vec::new()),
            location: Cell::new(NSPoint::new(0.0, 0.0)),
        });
        // SAFETY: the designated initializer for the superclass, with no target
        // until initialization completes. AppKit keeps target/delegate weak.
        let recognizer: Retained<Self> = unsafe {
            msg_send![super(this), initWithTarget: None::<&AnyObject>, action: None::<objc2::runtime::Sel>]
        };
        unsafe {
            recognizer.setTarget(Some(&recognizer));
            recognizer.setAction(Some(sel!(deliverTouches:)));
        }
        recognizer.setDelegate(Some(ProtocolObject::from_ref(&*recognizer)));
        recognizer.setAllowedTouchTypes(NSTouchTypeMask::Direct);
        recognizer
    }

    fn collect_touches(&self, native: &NSEvent, native_phase: NSTouchPhase, phase: TouchPhase) {
        let Some(view) = self.view() else {
            return;
        };
        let mut collected = false;
        for touch in native
            .touchesMatchingPhase_inView(native_phase, Some(&view))
            .iter()
        {
            if touch.r#type() != NSTouchType::Direct {
                continue;
            }
            let identity = touch.identity();
            let matches = self
                .ivars()
                .identity
                .borrow()
                .as_ref()
                .is_some_and(|active| {
                    // NSTouch identities implement NSObject's isEqual:, and may
                    // be reused after a contact ends. GPUI IDs are allocated anew.
                    unsafe { msg_send![&*identity, isEqual: &**active] }
                });
            if phase == TouchPhase::Started {
                if self.ivars().identity.borrow().is_some() {
                    continue;
                }
                *self.ivars().identity.borrow_mut() = Some(identity);
            } else if !matches {
                continue;
            }
            let location = touch.locationInView(Some(&view));
            self.ivars().location.set(location);
            let position = gpui_position(location, view.bounds().size.height, view.isFlipped());
            if let Some(event) = self.ivars().stream.borrow_mut().event(phase, position) {
                self.ivars().pending.borrow_mut().push(event);
                collected = true;
            }
            if matches!(phase, TouchPhase::Ended | TouchPhase::Cancelled) {
                self.ivars().identity.take();
            }
        }
        // A system cancellation may omit the contact. Still unwind GPUI's
        // active touch, so it can never produce a click or fling afterward.
        if phase == TouchPhase::Cancelled && !collected {
            let mut stream = self.ivars().stream.borrow_mut();
            let position = stream.position;
            if let Some(event) = stream.event(phase, position) {
                self.ivars().pending.borrow_mut().push(event);
                self.ivars().identity.take();
                collected = true;
            }
        }
        if collected {
            self.setState(match phase {
                TouchPhase::Started => NSGestureRecognizerState::Began,
                TouchPhase::Moved => NSGestureRecognizerState::Changed,
                TouchPhase::Ended => NSGestureRecognizerState::Ended,
                TouchPhase::Cancelled => NSGestureRecognizerState::Cancelled,
            });
        }
    }
}

pub fn configure_touch_scrolling(window: &mut Window, cx: &mut App) {
    if NSProcessInfo::processInfo()
        .operatingSystemVersion()
        .majorVersion
        < 27
    {
        return;
    }
    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return;
    };
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    // SAFETY: GPUI owns this live NSView; configuration runs on the main thread.
    let view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
    let recognizer = SidecarTouchRecognizer::new(mtm, window.to_async(cx));
    view.addGestureRecognizer(&recognizer);
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{
        Context, IntoElement, ListAlignment, ListState, ParentElement, Render, Styled,
        TestAppContext, div, list, prelude::*,
    };
    use std::{rc::Rc, time::Duration};

    struct Harness {
        list: ListState,
        clicks: Rc<Cell<usize>>,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let clicks = self.clicks.clone();
            div()
                .id("touch-harness")
                .size_full()
                .on_click(move |_, _, _| clicks.set(clicks.get() + 1))
                .child(
                    list(self.list.clone(), |_, _, _| {
                        div().h(px(20.0)).into_any_element()
                    })
                    .size_full(),
                )
        }
    }

    #[gpui::test]
    fn gpui_clicks_once_for_a_tap_and_coasts_after_a_pan_until_a_new_touch(
        cx: &mut TestAppContext,
    ) {
        let list =
            ListState::new(100, ListAlignment::Top, px(200.0)).with_uniform_item_height(px(20.0));
        let clicks = Rc::new(Cell::new(0));
        let (_, cx) = cx.add_window_view({
            let list = list.clone();
            let clicks = clicks.clone();
            move |_, _| Harness { list, clicks }
        });
        let mut stream = TouchStream::default();
        let start = point(px(100.0), px(200.0));
        cx.simulate_event(stream.event(TouchPhase::Started, start).unwrap());
        cx.simulate_event(stream.event(TouchPhase::Ended, start).unwrap());
        assert_eq!(clicks.get(), 1);

        cx.simulate_event(stream.event(TouchPhase::Started, start).unwrap());
        for y in [170.0, 140.0, 110.0] {
            // GPUI estimates velocity using real Instant samples, rather than
            // the test executor's virtual clock.
            std::thread::sleep(Duration::from_millis(16));
            cx.simulate_event(
                stream
                    .event(TouchPhase::Moved, point(px(100.0), px(y)))
                    .unwrap(),
            );
        }
        cx.simulate_event(stream.event(TouchPhase::Ended, stream.position).unwrap());
        let released = list.scroll_px_offset_for_scrollbar().y;
        assert!(released < px(0.0));
        assert_eq!(clicks.get(), 1, "a swipe must not click");
        std::thread::sleep(Duration::from_millis(16));
        cx.update(|window, cx| {
            window.simulate_next_frame(cx);
        });
        assert!(
            list.scroll_px_offset_for_scrollbar().y < released,
            "release must retain momentum"
        );

        cx.simulate_event(stream.event(TouchPhase::Started, start).unwrap());
        let caught = list.scroll_px_offset_for_scrollbar().y;
        std::thread::sleep(Duration::from_millis(16));
        cx.update(|window, cx| {
            window.simulate_next_frame(cx);
        });
        assert_eq!(list.scroll_px_offset_for_scrollbar().y, caught);
        cx.simulate_event(stream.event(TouchPhase::Cancelled, start).unwrap());
        assert_eq!(
            clicks.get(),
            1,
            "catching/cancelling a fling must not click"
        );
    }

    #[test]
    fn forwards_taps_and_pans_with_stable_unique_ids_and_unwinds_cancellation() {
        let mut stream = TouchStream::default();
        let position = point(px(40.0), px(100.0));
        let start = stream.event(TouchPhase::Started, position).unwrap();
        assert!(stream.event(TouchPhase::Started, position).is_none());
        let end = stream.event(TouchPhase::Ended, position).unwrap();
        assert_eq!(start.id, end.id);
        assert_eq!(end.position, position);
        assert!(stream.event(TouchPhase::Ended, position).is_none());
        let pan = stream.event(TouchPhase::Started, position).unwrap();
        assert_ne!(pan.id, start.id);
        let moved = stream
            .event(TouchPhase::Moved, point(px(40.0), px(200.0)))
            .unwrap();
        assert_eq!(moved.id, pan.id);
        let cancelled = stream.event(TouchPhase::Cancelled, moved.position).unwrap();
        assert_eq!(cancelled.id, pan.id);
        assert_eq!(cancelled.phase, TouchPhase::Cancelled);
        assert!(stream.event(TouchPhase::Moved, position).is_none());
    }

    #[test]
    fn converts_appkit_points_without_applying_the_display_scale() {
        let location = NSPoint::new(40.0, 100.0);
        assert_eq!(
            gpui_position(location, 800.0, false),
            point(px(40.0), px(700.0))
        );
        assert_eq!(
            gpui_position(location, 800.0, true),
            point(px(40.0), px(100.0))
        );
    }
}
