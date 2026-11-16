use super::*;
use gpui_kit::{Hsla, TestAppContext, component::ThemeMode};

fn contrast(first: Hsla, second: Hsla) -> f64 {
    let luminance = |color: Hsla| {
        let color = color.to_rgb();
        [color.r, color.g, color.b]
            .into_iter()
            .zip([0.2126, 0.7152, 0.0722])
            .map(|(channel, weight)| {
                let channel = f64::from(channel);
                weight
                    * if channel <= 0.04045 {
                        channel / 12.92
                    } else {
                        ((channel + 0.055) / 1.055).powf(2.4)
                    }
            })
            .sum::<f64>()
    };
    let first = luminance(first);
    let second = luminance(second);
    (first.max(second) + 0.05) / (first.min(second) + 0.05)
}

#[gpui_kit::test]
fn both_product_appearances_keep_text_and_focus_legible_when_switching_modes(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        init(cx);
        for mode in [ThemeMode::Light, ThemeMode::Dark, ThemeMode::Light] {
            Theme::change(mode, None, cx);
            let theme = Theme::global(cx);
            assert!(theme.theme_name().starts_with("Metronomo"));
            for (name, foreground, background) in [
                ("body", theme.foreground, theme.background),
                ("metadata", theme.muted_foreground, theme.background),
                (
                    "button",
                    theme.primary_foreground,
                    theme.tokens.button_primary.color,
                ),
                (
                    "button hover",
                    theme.primary_foreground,
                    theme.tokens.button_primary_hover.color,
                ),
                (
                    "button pressed",
                    theme.primary_foreground,
                    theme.tokens.button_primary_active.color,
                ),
            ] {
                assert!(
                    contrast(foreground, background) >= 4.5,
                    "{name} contrast in {mode:?}: {}",
                    contrast(foreground, background)
                );
            }
            assert!(contrast(theme.ring, theme.background) >= 3.0);
        }
    });
}
