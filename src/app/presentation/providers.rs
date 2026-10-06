use crate::model::ProviderKind;
use crate::theme::Theme;
use crate::ui::{icon, sp};
use gpui::{Div, Hsla, div, prelude::*, rgb};

/// Brand hue for each provider's official mark.
pub fn provider_color(theme: &Theme, provider: ProviderKind) -> Hsla {
    match provider {
        ProviderKind::Amp => rgb(0xF34E3F).into(),
        ProviderKind::Claude => rgb(0xD97757).into(),
        ProviderKind::DeepSeek => rgb(0x4D6BFE).into(),
        ProviderKind::Codex
        | ProviderKind::Cursor
        | ProviderKind::Fx
        | ProviderKind::OpenCode
        | ProviderKind::Grok
        | ProviderKind::Kimi
        | ProviderKind::OhMyPi
        | ProviderKind::Pi => {
            if theme.is_dark {
                rgb(0xF3F3F3).into()
            } else {
                rgb(0x34363B).into()
            }
        }
    }
}

/// Recognizable provider marks, matching the model picker vocabulary.
pub fn provider_icon(provider: ProviderKind) -> &'static str {
    match provider {
        ProviderKind::Amp => "icons/provider-amp.svg",
        ProviderKind::Claude => "icons/provider-claude.svg",
        ProviderKind::Codex => "icons/provider-openai.svg",
        ProviderKind::Cursor => "icons/provider-cursor.svg",
        ProviderKind::DeepSeek => "icons/provider-deepseek.svg",
        ProviderKind::Fx => "icons/provider-fx.svg",
        ProviderKind::OpenCode => "icons/provider-opencode.svg",
        ProviderKind::Grok => "icons/provider-grok.svg",
        ProviderKind::Kimi => "icons/provider-kimi.svg",
        ProviderKind::OhMyPi => "icons/provider-ohmypi.svg",
        ProviderKind::Pi => "icons/provider-pi.svg",
    }
}

/// A provider mark in a fixed square that never shrinks inside a row.
pub fn provider_mark(provider: ProviderKind, size: f32, color: Hsla) -> Div {
    div()
        .w(sp(size))
        .h(sp(size))
        .flex_none()
        .child(icon(provider_icon(provider), size, color))
}
