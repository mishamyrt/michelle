use std::{
    borrow::Cow,
    collections::HashMap,
    sync::{Arc, OnceLock},
};

use anyhow::Result;
use gpui::{App, AssetSource, SharedString};

mod symbols;

/// Brand marks stay embedded; ordinary icons are resolved from macOS once.
#[derive(Clone, Default)]
pub struct Assets {
    system_icons: Arc<OnceLock<HashMap<&'static str, Vec<u8>>>>,
}

impl Assets {
    pub fn prepare_system_icons(&self) -> Result<()> {
        let icons = symbols::load()?;
        self.system_icons
            .set(icons)
            .map_err(|_| anyhow::anyhow!("system icons already prepared"))
    }
}

macro_rules! icons {
    ($($name:literal),+ $(,)?) => {
        &[$((
            concat!("icons/", $name, ".svg"),
            include_bytes!(concat!("../assets/icons/", $name, ".svg")).as_slice(),
        )),+]
    };
}

const ICONS: &[(&str, &[u8])] = icons![
    "file-types/angular",
    "file-types/astro",
    "file-types/babel",
    "file-types/biome",
    "file-types/bun",
    "file-types/c",
    "file-types/clojure",
    "file-types/cmake",
    "file-types/coffee",
    "file-types/cpp",
    "file-types/crystal",
    "file-types/csharp",
    "file-types/css",
    "file-types/dart",
    "file-types/deno",
    "file-types/docker",
    "file-types/editorconfig",
    "file-types/elixir",
    "file-types/elm",
    "file-types/erlang",
    "file-types/eslint",
    "file-types/firebase",
    "file-types/git",
    "file-types/gitlab",
    "file-types/go",
    "file-types/gradle",
    "file-types/graphql",
    "file-types/haskell",
    "file-types/haxe",
    "file-types/helm",
    "file-types/html",
    "file-types/java",
    "file-types/javascript",
    "file-types/jinja",
    "file-types/json",
    "file-types/julia",
    "file-types/kotlin",
    "file-types/kubernetes",
    "file-types/lua",
    "file-types/makefile",
    "file-types/nest",
    "file-types/next",
    "file-types/nginx",
    "file-types/nix",
    "file-types/nodejs",
    "file-types/npm",
    "file-types/nuxt",
    "file-types/ocaml",
    "file-types/perl",
    "file-types/php",
    "file-types/pnpm",
    "file-types/powershell",
    "file-types/prettier",
    "file-types/prisma",
    "file-types/proto",
    "file-types/pug",
    "file-types/python",
    "file-types/react",
    "file-types/rollup",
    "file-types/ruby",
    "file-types/rust",
    "file-types/sass",
    "file-types/scala",
    "file-types/solidity",
    "file-types/storybook",
    "file-types/stylelint",
    "file-types/supabase",
    "file-types/svelte",
    "file-types/swift",
    "file-types/tailwindcss",
    "file-types/terraform",
    "file-types/turborepo",
    "file-types/typescript",
    "file-types/vite",
    "file-types/vitest",
    "file-types/vue",
    "file-types/webassembly",
    "file-types/webpack",
    "file-types/xaml",
    "file-types/xml",
    "file-types/yaml",
    "file-types/yarn",
    "file-types/zig",
    "github",
    "provider-amp",
    "provider-claude",
    "provider-cursor",
    "provider-deepseek",
    "provider-fx",
    "provider-grok",
    "provider-kimi",
    "provider-openai",
    "provider-ohmypi",
    "provider-opencode",
    "provider-pi",
];

const TEXT_FONTS: &[&[u8]] = &[
    include_bytes!("../assets/fonts/Lilex-Regular.ttf"),
    include_bytes!("../assets/fonts/Lilex-Bold.ttf"),
    include_bytes!("../assets/fonts/Lilex-Italic.ttf"),
    include_bytes!("../assets/fonts/Lilex-BoldItalic.ttf"),
];

/// Symbols-only icon face resolved via CoreText cascade (`FontFallbacks`),
/// never as a primary GPUI family; see `register_fonts_with_coretext`.
const SYMBOLS_FONT: &[u8] = include_bytes!("../assets/fonts/SymbolsNerdFontMono-Regular.ttf");

/// Family name of [`SYMBOLS_FONT`] for `FontFallbacks` lists.
pub const SYMBOLS_FONT_FAMILY: &str = "Symbols Nerd Font Mono";

pub fn register_fonts(cx: &App) -> Result<()> {
    cx.text_system().add_fonts(
        TEXT_FONTS
            .iter()
            .map(|font| Cow::Borrowed(*font))
            .collect::<Vec<_>>(),
    )?;
    crate::platform::register_fonts_with_coretext(&[SYMBOLS_FONT])
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = self.system_icons.get().and_then(|icons| icons.get(path)) {
            return Ok(Some(Cow::Owned(bytes.clone())));
        }
        Ok(ICONS
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(ICONS
            .iter()
            .map(|(name, _)| *name)
            .chain(symbols::SYMBOLS.iter().copied())
            .filter(|name| name.starts_with(path))
            .map(SharedString::from)
            .collect())
    }
}
