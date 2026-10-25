//! Native Mermaid previews. Frames read a bounded UI-owned cache; parsing,
//! layout, font loading and rasterization all run on background workers.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    AnyElement, App, DevicePixels, EntityId, Global, Hsla, IntoElement, ParentElement, RenderImage,
    RenderOnce, SharedString, Styled, SvgRenderer, SvgSize, Window, div, img, prelude::*, px,
    relative, size,
};
use merman::svg::{RenderResourcePolicy, SvgPipeline};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgEnvironment, SvgRequest};
use quick_xml::{Reader, events::Event};

use super::Palette;

const MAX_SOURCE_BYTES: usize = 64 * 1024;
const MAX_ENTRIES: usize = 64;
const MAX_CACHE_BYTES: usize = 32 * 1024 * 1024;
const MAX_PENDING: usize = 32;
const WORKERS: usize = 2;
const MAX_PIXELS: f32 = 4_000_000.0;

#[derive(IntoElement)]
pub(super) struct Diagram {
    pub source: SharedString,
    pub palette: Palette,
    pub fallback: AnyElement,
}

impl RenderOnce for Diagram {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let key = Key::new(self.source, &self.palette, window.scale_factor());
        match request(&key, window.current_view(), cx) {
            Some(rendered) => div()
                .w_full()
                .min_w_0()
                .flex()
                .justify_center()
                .child(
                    img(rendered.image.clone())
                        .w(px(rendered.width))
                        .max_w(relative(1.0))
                        .aspect_ratio(rendered.width / rendered.height)
                        .object_fit(gpui::ObjectFit::Contain),
                )
                .into_any_element(),
            None => self.fallback,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct Key {
    source: SharedString,
    dark: bool,
    text: u32,
    background: u32,
    scale: u32,
}

impl Key {
    fn new(source: SharedString, palette: &Palette, scale: f32) -> Self {
        fn rgba(color: Hsla) -> u32 {
            let color: gpui::Rgba = color.into();
            let [r, g, b, a] = [color.r, color.g, color.b, color.a]
                .map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u32);
            r << 24 | g << 16 | b << 8 | a
        }
        Self {
            source,
            dark: palette.is_dark,
            text: rgba(palette.text),
            background: rgba(palette.inset),
            scale: scale.clamp(1.0, 3.0).to_bits(),
        }
    }
}

struct Rendered {
    image: Arc<RenderImage>,
    width: f32,
    height: f32,
}

enum State {
    Pending(HashSet<EntityId>),
    // Cache failures too: malformed diagrams must not restart each frame.
    Ready(Option<Arc<Rendered>>),
}

struct Entry {
    state: State,
    touched: u64,
    bytes: usize,
}

#[derive(Default)]
struct Cache {
    entries: HashMap<Key, Entry>,
    queue: VecDeque<Key>,
    waiting: HashSet<EntityId>,
    clock: u64,
    active: usize,
    pending: usize,
    bytes: usize,
    retired: Vec<Arc<RenderImage>>,
}

#[derive(Default)]
struct Store(RefCell<Cache>);
impl Global for Store {}

impl Cache {
    fn request(&mut self, key: &Key, view: EntityId) -> Option<Arc<Rendered>> {
        if key.source.len() > MAX_SOURCE_BYTES || key.source.trim().is_empty() {
            return None;
        }
        self.clock += 1;
        if let Some(entry) = self.entries.get_mut(key) {
            entry.touched = self.clock;
            return match &mut entry.state {
                State::Pending(views) => {
                    views.insert(view);
                    None
                }
                State::Ready(rendered) => rendered.clone(),
            };
        }
        if self.pending >= MAX_PENDING {
            self.waiting.insert(view);
            return None;
        }
        self.entries.insert(
            key.clone(),
            Entry {
                state: State::Pending(HashSet::from([view])),
                touched: self.clock,
                bytes: 0,
            },
        );
        self.queue.push_back(key.clone());
        self.pending += 1;
        self.trim();
        None
    }

    fn trim(&mut self) {
        while self.entries.len() > MAX_ENTRIES || self.bytes > MAX_CACHE_BYTES {
            let oldest = self
                .entries
                .iter()
                .filter(|(_, entry)| matches!(entry.state, State::Ready(_)))
                .min_by_key(|(_, entry)| entry.touched)
                .map(|(key, _)| key.clone());
            let Some(key) = oldest else { break };
            let entry = self.entries.remove(&key).unwrap();
            self.bytes -= entry.bytes;
            if let State::Ready(Some(rendered)) = entry.state {
                self.retired.push(rendered.image.clone());
            }
        }
    }

    fn complete(&mut self, key: &Key, rendered: Option<Arc<Rendered>>) -> HashSet<EntityId> {
        let mut notify = std::mem::take(&mut self.waiting);
        // Source, theme and scale are immutable key parts. A superseded job
        // can only fill its own entry, never overwrite the current diagram.
        if let Some(entry) = self.entries.get_mut(key)
            && let State::Pending(views) = &mut entry.state
        {
            notify.extend(views.drain());
            entry.bytes = rendered
                .as_ref()
                .and_then(|r| r.image.as_bytes(0))
                .map_or(0, |b| b.len());
            self.bytes += entry.bytes;
            entry.state = State::Ready(rendered);
            self.pending -= 1;
        }
        self.trim();
        notify
    }
}

fn request(key: &Key, view: EntityId, cx: &mut App) -> Option<Arc<Rendered>> {
    if !cx.has_global::<Store>() {
        cx.set_global(Store::default());
    }
    let rendered = cx.global::<Store>().0.borrow_mut().request(key, view);
    retire_images(cx);
    pump(cx);
    rendered
}

fn retire_images(cx: &mut App) {
    let images = std::mem::take(&mut cx.global::<Store>().0.borrow_mut().retired);
    if !images.is_empty() {
        cx.defer(move |cx| {
            for image in images {
                cx.drop_image(image, None);
            }
        });
    }
}

fn pump(cx: &mut App) {
    loop {
        let key = {
            let mut cache = cx.global::<Store>().0.borrow_mut();
            if cache.active >= WORKERS {
                return;
            }
            let Some(key) = cache.queue.pop_front() else {
                return;
            };
            cache.active += 1;
            key
        };
        let renderer = cx.svg_renderer();
        let task = cx.background_executor().spawn(async move {
            let rendered = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                render(&key, &renderer).ok().map(Arc::new)
            }))
            .ok()
            .flatten();
            (key, rendered)
        });
        cx.spawn(async move |cx| {
            let (key, rendered) = task.await;
            let _ = cx.update(|cx| {
                let mut cache = cx.global::<Store>().0.borrow_mut();
                cache.active -= 1;
                let views = cache.complete(&key, rendered);
                drop(cache);
                for view in views {
                    cx.notify(view);
                }
                retire_images(cx);
                pump(cx);
            });
        })
        .detach();
    }
}

fn render(key: &Key, renderer: &SvgRenderer) -> anyhow::Result<Rendered> {
    anyhow::ensure!(
        key.source.len() <= MAX_SOURCE_BYTES && !key.source.trim().is_empty(),
        "Mermaid source limit"
    );
    let text = format!("#{:08x}", key.text);
    let background = format!("#{:08x}", key.background);
    let engine = merman::Engine::new().with_site_config(merman::MermaidConfig::from_value(
        serde_json::json!({
            "theme": if key.dark { "dark" } else { "default" },
            "layout": "dagre",
            "securityLevel": "strict",
            "fontFamily": "Arial, sans-serif",
            "themeVariables": {
                "background": background,
                "textColor": text,
                "titleColor": text,
                "edgeLabelBackground": background,
            },
        }),
    ));
    let output = Renderer::new()
        .with_engine(engine)
        .render(RenderRequest::svg(
            &key.source,
            OperationControl::new().with_deadline(Duration::from_secs(2)),
            SvgRequest {
                environment: SvgEnvironment::deterministic()
                    .with_resource_policy(RenderResourcePolicy::interactive()),
                pipeline: Some(SvgPipeline::resvg_safe()),
                ..Default::default()
            },
        ))?;
    let RenderOutput::Svg(Some(svg)) = output else {
        anyhow::bail!("No Mermaid diagram");
    };
    let (width, height) = svg_size(svg.svg())?;
    let scale = f32::from_bits(key.scale);
    // GPUI supersamples SVGs at 2x in addition to the display scale.
    let raster_scale = scale * 2.0;
    anyhow::ensure!(
        width <= 16_384.0 && height <= 16_384.0,
        "Mermaid image limit"
    );
    let raster_scale = raster_scale
        .min(8192.0 / width)
        .min(8192.0 / height)
        .min((MAX_PIXELS / width / height).sqrt());
    let parsed = renderer.parse_svg(svg.svg().as_bytes())?;
    let image = renderer.render_parsed(
        &parsed,
        SvgSize::ExactSize(size(
            DevicePixels((width * raster_scale).floor().max(1.0) as i32),
            DevicePixels((height * raster_scale).floor().max(1.0) as i32),
        )),
    )?;
    Ok(Rendered {
        image,
        width,
        height,
    })
}

fn svg_size(svg: &str) -> anyhow::Result<(f32, f32)> {
    let mut reader = Reader::from_str(svg);
    loop {
        match reader.read_event()? {
            Event::Start(tag) | Event::Empty(tag) if tag.name().as_ref() == "svg" => {
                for attribute in tag.attributes() {
                    let attribute = attribute?;
                    if attribute.key.as_ref() == "viewBox" {
                        let values = attribute
                            .value
                            .split_whitespace()
                            .map(str::parse::<f32>)
                            .collect::<Result<Vec<_>, _>>()?;
                        if let [x, y, width, height] = values.as_slice()
                            && [x, y, width, height].iter().all(|v| v.is_finite())
                            && *width > 0.0
                            && *height > 0.0
                        {
                            return Ok((*width, *height));
                        }
                    }
                }
                anyhow::bail!("Invalid Mermaid viewport");
            }
            Event::Eof => anyhow::bail!("Missing Mermaid viewport"),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    fn key(source: &str, dark: bool) -> Key {
        let theme = if dark { Theme::dark() } else { Theme::light() };
        Key::new(source.to_owned().into(), &Palette::from_theme(&theme), 2.0)
    }

    #[test]
    fn native_renderer_covers_common_diagrams_in_both_themes() {
        let renderer = SvgRenderer::new(Arc::new(()));
        for source in [
            "flowchart TD\n  A[Start] --> B{Ready?}\n  B -->|Yes| C[Done]",
            "sequenceDiagram\n  Alice->>Bob: Hello\n  Bob-->>Alice: Hi",
            "classDiagram\n  Animal <|-- Duck\n  Animal : +int age",
            "stateDiagram-v2\n  [*] --> Idle\n  Idle --> Running\n  Running --> [*]",
            "erDiagram\n  USER ||--o{ SESSION : owns",
            "pie title Tasks\n  \"Done\" : 8\n  \"Pending\" : 2",
            "flowchart LR\n  A[\"Привет\"] --> B[\"Готово\"]",
        ] {
            let light = render(&key(source, false), &renderer)
                .unwrap_or_else(|error| panic!("{source}: {error:?}"));
            let dark = render(&key(source, true), &renderer)
                .unwrap_or_else(|error| panic!("{source}: {error:?}"));
            assert!(light.width > 0.0 && light.height > 0.0);
            assert_eq!((light.width, light.height), (dark.width, dark.height));
            let pixels = light.image.as_bytes(0).unwrap();
            assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] > 0));
            assert!(pixels.len() <= MAX_PIXELS as usize * 4);
            assert_ne!(light.image.as_bytes(0), dark.image.as_bytes(0));
        }
    }

    #[test]
    fn invalid_and_oversized_sources_fall_back_without_panicking() {
        let renderer = SvgRenderer::new(Arc::new(()));
        for source in ["", "not a diagram", "flowchart TD\n A[Unfinished"] {
            assert!(render(&key(source, false), &renderer).is_err(), "{source}");
        }
        assert!(render(&key(&"x".repeat(MAX_SOURCE_BYTES + 1), false), &renderer).is_err());
        for svg in [
            "<svg/>",
            "<svg viewBox=\"0 0 NaN 20\"/>",
            "<svg viewBox=\"0 0 0 20\"/>",
            "<svg viewBox=\"0 0 -10 20\"/>",
        ] {
            assert!(svg_size(svg).is_err());
        }
        assert_eq!(
            svg_size("<svg viewBox=\"-10 -20 300 200\"/>").unwrap(),
            (300.0, 200.0)
        );
    }

    #[test]
    fn long_diagrams_stay_within_the_raster_budget_on_retina_displays() {
        let renderer = SvgRenderer::new(Arc::new(()));
        let mut source = String::from("flowchart TD\n");
        for i in 0..25 {
            source.push_str(&format!("N{i}[Step {i}] --> N{}\n", i + 1));
        }
        let rendered = render(&key(&source, false), &renderer).unwrap();
        assert!(rendered.height > 1000.0);
        assert!(rendered.image.as_bytes(0).unwrap().len() <= MAX_PIXELS as usize * 4);
    }

    #[test]
    fn cache_deduplicates_bounds_work_and_remembers_failures() {
        let mut cache = Cache::default();
        let key = key("flowchart TD\n A-->B", false);
        let view = EntityId::from(1);
        for _ in 0..1000 {
            assert!(cache.request(&key, view).is_none());
        }
        assert_eq!(cache.queue.len(), 1);
        assert_eq!(cache.pending, 1);
        assert_eq!(cache.complete(&key, None), HashSet::from([view]));
        cache.queue.clear();
        for _ in 0..1000 {
            assert!(cache.request(&key, view).is_none());
        }
        assert!(cache.queue.is_empty());
        for i in 0..MAX_PENDING * 2 {
            cache.request(
                &Key {
                    source: format!("flowchart TD\n A-->B{i}").into(),
                    ..key.clone()
                },
                view,
            );
        }
        assert_eq!(cache.pending, MAX_PENDING);
        assert_eq!(cache.queue.len(), MAX_PENDING);
        assert_eq!(cache.waiting, HashSet::from([view]));
    }

    #[test]
    fn stale_results_cannot_replace_new_source_theme_or_scale() {
        let mut cache = Cache::default();
        let old = key("flowchart TD\n A-->B", false);
        let new = key("flowchart TD\n A-->C", false);
        let dark = key("flowchart TD\n A-->B", true);
        let scaled = Key {
            scale: 1.0_f32.to_bits(),
            ..old.clone()
        };
        let view = EntityId::from(1);
        for key in [&old, &new, &dark, &scaled] {
            cache.request(key, view);
        }
        cache.complete(&old, None);
        assert!(matches!(cache.entries[&old].state, State::Ready(None)));
        for key in [&new, &dark, &scaled] {
            assert!(matches!(cache.entries[key].state, State::Pending(_)));
        }
        for i in 0..MAX_ENTRIES * 2 {
            let next = Key {
                source: format!("flowchart TD\n A-->B{i}").into(),
                ..old.clone()
            };
            cache.request(&next, view);
            cache.complete(&next, None);
        }
        assert_eq!(cache.entries.len(), MAX_ENTRIES);
        assert_eq!(cache.pending, 3);
    }

    #[test]
    #[ignore = "manual performance measurement"]
    fn benchmark_mermaid_cache() {
        let renderer = SvgRenderer::new(Arc::new(()));
        let key = key(
            "flowchart TD\n A[Start] --> B{Ready?}\n B -->|Yes| C[Done]",
            false,
        );
        let started = std::time::Instant::now();
        let rendered = Arc::new(render(&key, &renderer).unwrap());
        let cold = started.elapsed();
        let mut cache = Cache::default();
        let view = EntityId::from(1);
        cache.request(&key, view);
        cache.queue.clear();
        cache.complete(&key, Some(rendered));
        let started = std::time::Instant::now();
        for _ in 0..100_000 {
            std::hint::black_box(cache.request(&key, view));
        }
        let warm = started.elapsed();
        assert!(cache.queue.is_empty());
        eprintln!(
            "Mermaid: cold layout+raster {cold:?}; cached lookup {:?}",
            warm / 100_000
        );
    }
}
