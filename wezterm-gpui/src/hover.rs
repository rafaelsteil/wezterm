//! Hyperlink hover identity + underline spans (058).
//!
//! wezterm-gui stores `current_highlight: Option<Arc<Hyperlink>>` and matches
//! with `Arc::ptr_eq`. Consecutive matching cells get a Single underline.
//! GPUI overlays a quad instead of baking into 017 line sprites.

use std::sync::Arc;

use wezterm_term::{Hyperlink, Line};

/// Consecutive cells of the hovered hyperlink. Overlay, not row-sprite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkHoverSpan {
    pub vis_row: usize,
    pub col: usize,
    pub len: usize,
}

/// Same identity as wezterm-gui `same_hyperlink` (`Arc::ptr_eq`).
pub fn same_hyperlink(a: Option<&Arc<Hyperlink>>, b: Option<&Arc<Hyperlink>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

/// wezterm-gui default InputMap: unmodified/SHIFT Left Up is
/// `CompleteSelectionOrOpenLinkAtMouseCursor`. User lua may *add* Ctrl+click
/// `OpenLinkAtMouseCursor`; that does not disable the default unless
/// `disable_default_mouse_bindings` is set. Pass that flag (or a chord that
/// is not the default binding) as `default_blocked`.
pub fn click_opens_hovered_link(
    lua_opens: bool,
    lua_complete_or_open: bool,
    lua_nop: bool,
    has_selection: bool,
    default_blocked: bool,
) -> bool {
    if lua_nop {
        return false;
    }
    if lua_opens {
        return true;
    }
    if lua_complete_or_open {
        return !has_selection;
    }
    !has_selection && !default_blocked
}

/// Consecutive cells whose hyperlink Arc matches the hover (one run per row).
pub fn hover_underline_spans(
    lines: &[Line],
    highlight: Option<&Arc<Hyperlink>>,
) -> Vec<LinkHoverSpan> {
    let Some(want) = highlight else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (vis_row, line) in lines.iter().enumerate() {
        let mut run_start: Option<usize> = None;
        let mut run_end = 0usize;
        for cell in line.visible_cells() {
            let col = cell.cell_index();
            let hit = cell
                .attrs()
                .hyperlink()
                .is_some_and(|h| Arc::ptr_eq(h, want));
            if hit {
                if run_start.is_none() {
                    run_start = Some(col);
                }
                run_end = col + cell.width().max(1);
            } else if let Some(start) = run_start.take() {
                out.push(LinkHoverSpan {
                    vis_row,
                    col: start,
                    len: (run_end - start).max(1),
                });
            }
        }
        if let Some(start) = run_start {
            out.push(LinkHoverSpan {
                vis_row,
                col: start,
                len: (run_end - start).max(1),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_with_link(text: &str, start: usize, end: usize, link: &Arc<Hyperlink>) -> Line {
        let mut line: Line = text.into();
        for cell in &mut line.cells_mut()[start..end] {
            cell.attrs_mut().set_hyperlink(Some(Arc::clone(link)));
        }
        line
    }

    #[test]
    fn hover_spans_one_url_run() {
        let link = Arc::new(Hyperlink::new_implicit("http://example.com"));
        let line = line_with_link("see http://example.com now", 4, 22, &link);
        let spans = hover_underline_spans(&[line], Some(&link));
        assert_eq!(
            spans,
            vec![LinkHoverSpan {
                vis_row: 0,
                col: 4,
                len: 18,
            }]
        );
    }

    #[test]
    fn hover_spans_ignore_other_arc() {
        let a = Arc::new(Hyperlink::new_implicit("http://example.com"));
        let b = Arc::new(Hyperlink::new_implicit("http://example.com"));
        let line = line_with_link("http://example.com", 0, 18, &a);
        assert!(hover_underline_spans(&[line], Some(&b)).is_empty());
        assert!(!same_hyperlink(Some(&a), Some(&b)));
        assert!(same_hyperlink(Some(&a), Some(&a)));
    }

    #[test]
    fn hover_none_is_empty() {
        let link = Arc::new(Hyperlink::new_implicit("http://example.com"));
        let line = line_with_link("http://example.com", 0, 18, &link);
        assert!(hover_underline_spans(&[line], None).is_empty());
    }

    #[test]
    fn plain_click_opens_when_no_selection() {
        assert!(click_opens_hovered_link(false, false, false, false, false));
        assert!(!click_opens_hovered_link(false, false, false, true, false));
        assert!(!click_opens_hovered_link(false, false, false, false, true));
    }

    #[test]
    fn lua_ctrl_click_still_opens() {
        assert!(click_opens_hovered_link(true, false, false, false, true));
        assert!(!click_opens_hovered_link(false, false, true, false, true));
    }

    #[test]
    fn complete_or_open_skips_when_selecting() {
        assert!(click_opens_hovered_link(false, true, false, false, false));
        assert!(!click_opens_hovered_link(false, true, false, true, false));
    }

    #[test]
    fn disable_default_skips_plain_click_keeps_lua() {
        assert!(!click_opens_hovered_link(false, false, false, false, true));
        assert!(click_opens_hovered_link(true, false, false, false, true));
    }
}
