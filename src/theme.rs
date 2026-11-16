//! Product colors live here; feature views consume the active theme's semantic roles.

use gpui_kit::{
    App,
    component::{Theme, ThemeRegistry},
};

/// Register both appearances so system appearance changes retain the product palette.
pub(crate) fn init(cx: &mut App) {
    let result =
        ThemeRegistry::global_mut(cx).load_themes_from_str(include_str!("theme/metronomo.json"));
    if let Err(error) = result {
        eprintln!("Could not load the Metronomo theme: {error}");
        return;
    }
    let themes = ThemeRegistry::global(cx).themes();
    let Some(light) = themes.get("Metronomo Light").cloned() else {
        return;
    };
    let Some(dark) = themes.get("Metronomo Dark").cloned() else {
        return;
    };
    Theme::update(cx, |theme| {
        theme.light_theme = light;
        theme.dark_theme = dark;
    });
    let mode = Theme::global(cx).mode;
    Theme::change(mode, None, cx);
}

#[cfg(test)]
mod tests;
