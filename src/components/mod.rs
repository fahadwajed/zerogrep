pub mod about;
pub mod dialog;
pub mod filter_dialog;
pub mod filter_row;

use gtk::{gdk, pango};

use crate::core::types::RowStyle;
pub fn rgba_to_pango(c: &gdk::RGBA) -> (u16, u16, u16) {
    (
        (c.red().clamp(0.0, 1.0) * 65535.0) as u16,
        (c.green().clamp(0.0, 1.0) * 65535.0) as u16,
        (c.blue().clamp(0.0, 1.0) * 65535.0) as u16,
    )
}

pub fn pango_to_rgba(c: (u16, u16, u16)) -> gdk::RGBA {
    gdk::RGBA::new(
        c.0 as f32 / 65535.0,
        c.1 as f32 / 65535.0,
        c.2 as f32 / 65535.0,
        1.0,
    )
}

pub fn build_pango_attrs(style: Option<RowStyle>) -> Option<pango::AttrList> {
    let s = style?;
    if s.fg.is_none() && s.bg.is_none() {
        return None;
    }
    let attrs = pango::AttrList::new();
    if let Some((r, g, b)) = s.fg {
        attrs.insert(pango::AttrColor::new_foreground(r, g, b));
    }
    if let Some((r, g, b)) = s.bg {
        attrs.insert(pango::AttrColor::new_background(r, g, b));
    }
    Some(attrs)
}

static THEME_IS_DARK: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

fn theme_is_dark() -> bool {
    *THEME_IS_DARK.get_or_init(|| {
        std::panic::catch_unwind(|| adw::StyleManager::default().is_dark())
            .unwrap_or(true)
    })
}

pub(crate) fn blend_tint(c: u16, b: u16) -> u16 {
    ((u32::from(c) * 15 + u32::from(b) * 85) / 100) as u16
}

pub fn build_line_attrs(
    style: Option<RowStyle>,
    text: &str,
    span: Option<(usize, usize)>,
    context_color: Option<(u16, u16, u16)>,
) -> Option<pango::AttrList> {
    let attrs = pango::AttrList::new();
    let mut any = false;

    if let Some(s) = style {
        if let Some((r, g, b)) = s.fg {
            attrs.insert(pango::AttrColor::new_foreground(r, g, b));
            any = true;
        }
        if let Some((r, g, b)) = s.bg {
            attrs.insert(pango::AttrColor::new_background(r, g, b));
            any = true;
        }
    }

    if let Some((start, end)) = span
        && start < end
        && text.is_char_boundary(start)
        && text.is_char_boundary(end)
    {
        let start_idx = text[..start].chars().count() as u32;
        let end_idx = text[..end].chars().count() as u32;

        let mut bg = pango::AttrColor::new_background(0xffff, 0xd4d4, 0x3b3b);
        bg.set_start_index(start_idx);
        bg.set_end_index(end_idx);
        attrs.insert(bg);
        let mut fg = pango::AttrColor::new_foreground(0x2424, 0x1a1a, 0x0000);
        fg.set_start_index(start_idx);
        fg.set_end_index(end_idx);
        attrs.insert(fg);
        any = true;
    } else if let Some(color) = context_color {
        let dark = theme_is_dark();
        let base: (u16, u16, u16) = if dark {
            (0x2424, 0x2424, 0x2828)
        } else {
            (0xfaf6, 0xfaf5, 0xfaf4)
        };
        attrs.insert(pango::AttrColor::new_background(
            blend_tint(color.0, base.0),
            blend_tint(color.1, base.1),
            blend_tint(color.2, base.2),
        ));
        any = true;
    }

    if any {
        Some(attrs)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::blend_tint;

    #[test]
    fn tint_never_overflows_and_stays_between_the_inputs() {
        let bright = (u16::MAX, u16::MAX, u16::MAX);
        for (c, b) in [
            (bright.0, 0x2424),
            (bright.0, 0xfaf6),
            (0, 0),
            (u16::MAX, u16::MAX),
            (0x8a8a, 0x2424),
        ] {
            let t = blend_tint(c, b);
            let lo = u16::min(c, b);
            let hi = u16::max(c, b);
            assert!((lo..=hi).contains(&t), "tint {t} outside [{lo},{hi}]");
        }
    }

    #[test]
    fn tint_is_mostly_the_base() {
        let t = blend_tint(0xffff, 0x2424);
        assert_eq!(t, 0x451e);
        assert!(t < 0x5000, "expected a mostly-dark tint, got {t:#x}");
    }
}
