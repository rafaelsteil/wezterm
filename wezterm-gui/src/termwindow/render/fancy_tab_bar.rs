use crate::customglyph::*;
use crate::tabbar::{TabBarItem, TabEntry};
use crate::termwindow::box_model::*;
use crate::termwindow::render::corners::*;

use crate::termwindow::render::window_buttons::window_button_element;
use crate::termwindow::{UIItem, UIItemType};
use crate::utilsprites::RenderMetrics;
use config::{Dimension, DimensionContext, TabBarColors};
use std::rc::Rc;
use wezterm_font::LoadedFont;
use wezterm_term::color::{ColorAttribute, ColorPalette};
use window::color::LinearRgba;
use window::{IntegratedTitleButtonAlignment, IntegratedTitleButtonStyle, RectF};

const X_BUTTON: &[Poly] = &[
    Poly {
        path: &[
            PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Zero),
            PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
        ],
        intensity: BlockAlpha::Full,
        style: PolyStyle::Outline,
    },
    Poly {
        path: &[
            PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
            PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
        ],
        intensity: BlockAlpha::Full,
        style: PolyStyle::Outline,
    },
];

const PLUS_BUTTON: &[Poly] = &[
    Poly {
        path: &[
            PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
            PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
        ],
        intensity: BlockAlpha::Full,
        style: PolyStyle::Outline,
    },
    Poly {
        path: &[
            PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
            PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
        ],
        intensity: BlockAlpha::Full,
        style: PolyStyle::Outline,
    },
];

impl crate::TermWindow {
    pub fn invalidate_fancy_tab_bar(&mut self) {
        self.fancy_tab_bar.take();
    }

    pub fn build_fancy_tab_bar(&mut self, palette: &ColorPalette) -> anyhow::Result<ComputedElement> {
        let tab_bar_height = self.tab_bar_pixel_height()?;
        let font = self.fonts.title_font()?;
        let metrics = RenderMetrics::with_font_metrics(&font.metrics());
        let items = self.tab_bar.items();
        let colors = self
            .config
            .colors
            .as_ref()
            .and_then(|c| c.tab_bar.as_ref())
            .cloned()
            .unwrap_or_else(TabBarColors::default);

        let mut left_status = vec![];
        let mut left_eles = vec![];
        let mut new_tab_eles = vec![];
        let mut right_eles = vec![];
        let bar_colors = ElementColors {
            border: BorderColor::default(),
            bg: if self.focused.is_some() {
                self.config.window_frame.active_titlebar_bg
            } else {
                self.config.window_frame.inactive_titlebar_bg
            }
            .to_linear()
            .into(),
            text: if self.focused.is_some() {
                self.config.window_frame.active_titlebar_fg
            } else {
                self.config.window_frame.inactive_titlebar_fg
            }
            .to_linear()
            .into(),
        };

        let item_to_elem = |item: &TabEntry| -> Element {
            let element = Element::with_line(&font, &item.title, palette);

            let bg_color = item
                .title
                .get_cell(0)
                .and_then(|c| match c.attrs().background() {
                    ColorAttribute::Default => None,
                    col => Some(palette.resolve_bg(col)),
                });
            let fg_color = item
                .title
                .get_cell(0)
                .and_then(|c| match c.attrs().foreground() {
                    ColorAttribute::Default => None,
                    col => Some(palette.resolve_fg(col)),
                });

            let new_tab = colors.new_tab();
            let new_tab_hover = colors.new_tab_hover();
            let active_tab = colors.active_tab();

            match item.item {
                TabBarItem::RightStatus | TabBarItem::LeftStatus | TabBarItem::None => element
                    .item_type(UIItemType::TabBar(TabBarItem::None))
                    .line_height(Some(1.75))
                    .margin(BoxDimension {
                        left: Dimension::Cells(0.),
                        right: Dimension::Cells(0.),
                        top: Dimension::Cells(0.0),
                        bottom: Dimension::Cells(0.),
                    })
                    .padding(BoxDimension {
                        left: Dimension::Cells(0.5),
                        right: Dimension::Cells(0.),
                        top: Dimension::Cells(0.),
                        bottom: Dimension::Cells(0.),
                    })
                    .border(BoxDimension::new(Dimension::Pixels(0.)))
                    .colors(bar_colors.clone()),
                TabBarItem::NewTabButton => Element::new(
                    &font,
                    ElementContent::Poly {
                        line_width: metrics.underline_height.max(2),
                        poly: SizedPoly {
                            poly: PLUS_BUTTON,
                            width: Dimension::Pixels(metrics.cell_size.height as f32 / 2.),
                            height: Dimension::Pixels(metrics.cell_size.height as f32 / 2.),
                        },
                    },
                )
                .vertical_align(VerticalAlign::Middle)
                .item_type(UIItemType::TabBar(item.item.clone()))
                .margin(BoxDimension {
                    left: Dimension::Cells(0.5),
                    right: Dimension::Cells(0.),
                    top: Dimension::Cells(0.2),
                    bottom: Dimension::Cells(0.),
                })
                .padding(BoxDimension {
                    left: Dimension::Cells(0.5),
                    right: Dimension::Cells(0.5),
                    top: Dimension::Cells(0.2),
                    bottom: Dimension::Cells(0.25),
                })
                .border(BoxDimension::new(Dimension::Pixels(1.)))
                .colors(ElementColors {
                    border: BorderColor::default(),
                    bg: new_tab.bg_color.to_linear().into(),
                    text: new_tab.fg_color.to_linear().into(),
                })
                .hover_colors(Some(ElementColors {
                    border: BorderColor::default(),
                    bg: new_tab_hover.bg_color.to_linear().into(),
                    text: new_tab_hover.fg_color.to_linear().into(),
                })),
                TabBarItem::Tab { active, .. } if active => element
                    .vertical_align(VerticalAlign::Bottom)
                    .item_type(UIItemType::TabBar(item.item.clone()))
                    .margin(BoxDimension {
                        left: Dimension::Cells(0.),
                        right: Dimension::Cells(0.),
                        top: Dimension::Cells(0.2),
                        bottom: Dimension::Cells(0.),
                    })
                    .padding(BoxDimension {
                        left: Dimension::Cells(0.5),
                        right: Dimension::Cells(0.5),
                        top: Dimension::Cells(0.2),
                        bottom: Dimension::Cells(0.25),
                    })
                    .border(BoxDimension::new(Dimension::Pixels(1.)))
                    .border_corners(Some(Corners {
                        top_left: SizedPoly {
                            width: Dimension::Cells(0.5),
                            height: Dimension::Cells(0.5),
                            poly: TOP_LEFT_ROUNDED_CORNER,
                        },
                        top_right: SizedPoly {
                            width: Dimension::Cells(0.5),
                            height: Dimension::Cells(0.5),
                            poly: TOP_RIGHT_ROUNDED_CORNER,
                        },
                        bottom_left: SizedPoly::none(),
                        bottom_right: SizedPoly::none(),
                    }))
                    .colors(ElementColors {
                        border: BorderColor::new(
                            bg_color
                                .unwrap_or_else(|| active_tab.bg_color.into())
                                .to_linear(),
                        ),
                        bg: bg_color
                            .unwrap_or_else(|| active_tab.bg_color.into())
                            .to_linear()
                            .into(),
                        text: fg_color
                            .unwrap_or_else(|| active_tab.fg_color.into())
                            .to_linear()
                            .into(),
                    }),
                TabBarItem::Tab { .. } => element
                    .vertical_align(VerticalAlign::Bottom)
                    .item_type(UIItemType::TabBar(item.item.clone()))
                    .margin(BoxDimension {
                        left: Dimension::Cells(0.),
                        right: Dimension::Cells(0.),
                        top: Dimension::Cells(0.2),
                        bottom: Dimension::Cells(0.),
                    })
                    .padding(BoxDimension {
                        left: Dimension::Cells(0.5),
                        right: Dimension::Cells(0.5),
                        top: Dimension::Cells(0.2),
                        bottom: Dimension::Cells(0.25),
                    })
                    .border(BoxDimension::new(Dimension::Pixels(1.)))
                    .border_corners(Some(Corners {
                        top_left: SizedPoly {
                            width: Dimension::Cells(0.5),
                            height: Dimension::Cells(0.5),
                            poly: TOP_LEFT_ROUNDED_CORNER,
                        },
                        top_right: SizedPoly {
                            width: Dimension::Cells(0.5),
                            height: Dimension::Cells(0.5),
                            poly: TOP_RIGHT_ROUNDED_CORNER,
                        },
                        bottom_left: SizedPoly {
                            width: Dimension::Cells(0.),
                            height: Dimension::Cells(0.33),
                            poly: &[],
                        },
                        bottom_right: SizedPoly {
                            width: Dimension::Cells(0.),
                            height: Dimension::Cells(0.33),
                            poly: &[],
                        },
                    }))
                    .colors({
                        let inactive_tab = colors.inactive_tab();
                        let bg = bg_color
                            .unwrap_or_else(|| inactive_tab.bg_color.into())
                            .to_linear();
                        let edge = colors.inactive_tab_edge().to_linear();
                        ElementColors {
                            border: BorderColor {
                                left: bg,
                                right: edge,
                                top: bg,
                                bottom: bg,
                            },
                            bg: bg.into(),
                            text: fg_color
                                .unwrap_or_else(|| inactive_tab.fg_color.into())
                                .to_linear()
                                .into(),
                        }
                    })
                    .hover_colors({
                        let inactive_tab_hover = colors.inactive_tab_hover();
                        Some(ElementColors {
                            border: BorderColor::new(
                                bg_color
                                    .unwrap_or_else(|| inactive_tab_hover.bg_color.into())
                                    .to_linear(),
                            ),
                            bg: bg_color
                                .unwrap_or_else(|| inactive_tab_hover.bg_color.into())
                                .to_linear()
                                .into(),
                            text: fg_color
                                .unwrap_or_else(|| inactive_tab_hover.fg_color.into())
                                .to_linear()
                                .into(),
                        })
                    }),
                TabBarItem::WindowButton(button) => window_button_element(
                    button,
                    self.window_state.contains(window::WindowState::MAXIMIZED),
                    &font,
                    &metrics,
                    &self.config,
                ),
            }
        };

        let max_tab_width =
            (self.config.tab_max_width as f32 * metrics.cell_size.width as f32).max(0.);

        // Reserve space for the native titlebar buttons
        if self
            .config
            .window_decorations
            .contains(::window::WindowDecorations::INTEGRATED_BUTTONS)
            && self.config.integrated_title_button_style == IntegratedTitleButtonStyle::MacOsNative
            && !self.window_state.contains(window::WindowState::FULL_SCREEN)
        {
            left_status.push(
                Element::new(&font, ElementContent::Text("".to_string())).margin(BoxDimension {
                    left: Dimension::Cells(4.0), // FIXME: determine exact width of macos ... buttons
                    right: Dimension::Cells(0.),
                    top: Dimension::Cells(0.),
                    bottom: Dimension::Cells(0.),
                }),
            );
        }

        for item in items {
            match item.item {
                TabBarItem::LeftStatus => left_status.push(item_to_elem(item)),
                TabBarItem::None | TabBarItem::RightStatus => right_eles.push(item_to_elem(item)),
                TabBarItem::WindowButton(_) => {
                    if self.config.integrated_title_button_alignment
                        == IntegratedTitleButtonAlignment::Left
                    {
                        left_status.push(item_to_elem(item))
                    } else {
                        right_eles.push(item_to_elem(item))
                    }
                }
                TabBarItem::Tab { tab_idx, active } => {
                    let mut elem = item_to_elem(item);
                    elem.max_width = Some(Dimension::Pixels(max_tab_width));
                    elem.content = match elem.content {
                        ElementContent::Text(_) => unreachable!(),
                        ElementContent::Poly { .. } => unreachable!(),
                        ElementContent::Children(mut kids) => {
                            if self.config.show_close_tab_button_in_tabs {
                                kids.push(make_x_button(&font, &metrics, &colors, tab_idx, active));
                            }
                            ElementContent::Children(kids)
                        }
                    };
                    left_eles.push(elem);
                }
                TabBarItem::NewTabButton => new_tab_eles.push(item_to_elem(item)),
            }
        }

        let mut children = vec![];

        if !left_status.is_empty() {
            children.push(
                Element::new(&font, ElementContent::Children(left_status))
                    .colors(bar_colors.clone()),
            );
        }

        let window_buttons_at_left = self
            .config
            .window_decorations
            .contains(window::WindowDecorations::INTEGRATED_BUTTONS)
            && (self.config.integrated_title_button_alignment
                == IntegratedTitleButtonAlignment::Left
                || self.config.integrated_title_button_style
                    == IntegratedTitleButtonStyle::MacOsNative);

        let left_padding = if window_buttons_at_left {
            if self.config.integrated_title_button_style == IntegratedTitleButtonStyle::MacOsNative
            {
                if !self.window_state.contains(window::WindowState::FULL_SCREEN) {
                    Dimension::Pixels(70.0)
                } else {
                    Dimension::Cells(0.5)
                }
            } else {
                Dimension::Pixels(0.0)
            }
        } else {
            Dimension::Cells(0.5)
        };

        children.push(
            Element::new(&font, ElementContent::Children(left_eles))
                .vertical_align(VerticalAlign::Bottom)
                .colors(ElementColors {
                    border: BorderColor::default(),
                    bg: LinearRgba::TRANSPARENT.into(),
                    text: bar_colors.text.clone(),
                })
                .padding(BoxDimension {
                    left: left_padding,
                    right: Dimension::Cells(0.),
                    top: Dimension::Cells(0.),
                    bottom: Dimension::Cells(0.),
                })
                .allow_overflow(true)
                .zindex(1),
        );
        if !new_tab_eles.is_empty() {
            children.push(
                Element::new(&font, ElementContent::Children(new_tab_eles))
                    .vertical_align(VerticalAlign::Bottom)
                    .colors(ElementColors {
                        border: BorderColor::default(),
                        bg: LinearRgba::TRANSPARENT.into(),
                        text: bar_colors.text.clone(),
                    })
                    .zindex(2),
            );
        }
        children.push(
            Element::new(&font, ElementContent::Children(right_eles))
                .colors(bar_colors.clone())
                .float(Float::Right),
        );

        let content = ElementContent::Children(children);

        let tabs = Element::new(&font, content)
            .display(DisplayType::Block)
            .item_type(UIItemType::TabBar(TabBarItem::None))
            .min_width(Some(Dimension::Pixels(self.dimensions.pixel_width as f32)))
            .min_height(Some(Dimension::Pixels(tab_bar_height)))
            .vertical_align(VerticalAlign::Bottom)
            .colors(bar_colors);

        let border = self.get_os_border();

        let mut computed = self.compute_element(
            &LayoutContext {
                height: DimensionContext {
                    dpi: self.dimensions.dpi as f32,
                    pixel_max: self.dimensions.pixel_height as f32,
                    pixel_cell: metrics.cell_size.height as f32,
                },
                width: DimensionContext {
                    dpi: self.dimensions.dpi as f32,
                    pixel_max: self.dimensions.pixel_width as f32,
                    pixel_cell: metrics.cell_size.width as f32,
                },
                bounds: euclid::rect(
                    border.left.get() as f32,
                    0.,
                    self.dimensions.pixel_width as f32 - (border.left + border.right).get() as f32,
                    tab_bar_height,
                ),
                metrics: &metrics,
                gl_state: self.render_state.as_ref().unwrap(),
                zindex: 10,
            },
            &tabs,
        )?;

        computed.translate(euclid::vec2(
            0.,
            if self.config.tab_bar_at_bottom {
                self.dimensions.pixel_height as f32
                    - (computed.bounds.height() + border.bottom.get() as f32)
            } else {
                border.top.get() as f32
            },
        ));

        self.apply_fancy_tab_strip_scroll(&mut computed);

        Ok(computed)
    }

    pub fn paint_fancy_tab_bar(&self) -> anyhow::Result<Vec<UIItem>> {
        let computed = self.fancy_tab_bar.as_ref().ok_or_else(|| {
            anyhow::anyhow!("paint_fancy_tab_bar called but fancy_tab_bar is None")
        })?;
        let mut ui_items = computed.ui_items();
        if let Some(clip) = self.tab_bar_strip_clip {
            ui_items = clip_tab_strip_ui_items(ui_items, clip);
        }

        let gl_state = self.render_state.as_ref().unwrap();
        self.render_element(&computed, gl_state, None, None)?;

        Ok(ui_items)
    }

    pub fn pan_tab_bar(&mut self, delta_pixels: f32, context: &dyn ::window::WindowOps) {
        let next = (self.tab_bar_scroll_offset + delta_pixels).clamp(0., self.tab_bar_scroll_max);
        let applied = next - self.tab_bar_scroll_offset;
        if applied == 0. {
            return;
        }
        self.tab_bar_scroll_offset = next;
        if let Some(computed) = self.fancy_tab_bar.as_mut() {
            if let Some(idx) = scrolling_tab_strip_index(computed) {
                if let ComputedElementContent::Children(kids) = &mut computed.content {
                    kids[idx].translate(euclid::vec2(-applied, 0.));
                }
            }
        }
        context.invalidate();
    }

    fn apply_fancy_tab_strip_scroll(&mut self, computed: &mut ComputedElement) {
        self.tab_bar_strip_clip = None;
        self.tab_bar_scroll_max = 0.;

        let Some(strip_idx) = scrolling_tab_strip_index(computed) else {
            self.tab_bar_scroll_offset = 0.;
            self.tab_bar_follow_active = false;
            return;
        };
        let plus_idx = new_tab_button_index(computed);
        let right_left = find_right_chrome_left(computed).unwrap_or_else(|| {
            let border = self.get_os_border();
            self.dimensions.pixel_width as f32 - border.right.get() as f32
        });
        let computed_height = computed.bounds.height();

        let (viewport_left, content_width, strip_y, strip_h) = match &computed.content {
            ComputedElementContent::Children(kids) => {
                let strip = &kids[strip_idx];
                (
                    strip.bounds.min_x(),
                    strip.bounds.width(),
                    strip.bounds.min_y(),
                    strip.bounds.height().max(computed_height),
                )
            }
            _ => return,
        };

        let plus_width = plus_idx
            .and_then(|i| match &computed.content {
                ComputedElementContent::Children(kids) => Some(new_tab_visual_width(&kids[i])),
                _ => None,
            })
            .unwrap_or(0.);

        // Layout padding is squeezed when the tab strip overflows, so reserve
        // the drag handle in this pin instead.
        let drag_gap = self.tab_bar_drag_gap_px();
        let max_plus_x = (right_left - drag_gap - plus_width).max(viewport_left);
        let plus_x = if plus_width > 0. {
            (viewport_left + content_width).min(max_plus_x)
        } else {
            right_left
        };

        if let Some(i) = plus_idx {
            if let ComputedElementContent::Children(kids) = &mut computed.content {
                let dx = plus_x - kids[i].bounds.min_x();
                if dx != 0. {
                    kids[i].translate(euclid::vec2(dx, 0.));
                }
            }
        }

        let viewport_width = (plus_x - viewport_left).max(0.);
        let max_scroll = (content_width - viewport_width).max(0.);

        let strip = match &mut computed.content {
            ComputedElementContent::Children(kids) => &mut kids[strip_idx],
            _ => return,
        };

        if self.tab_bar_follow_active {
            if let Some(active) = find_active_tab_bounds(strip) {
                let local_start = active.min_x() - viewport_left;
                let local_end = active.max_x() - viewport_left;
                if local_start < self.tab_bar_scroll_offset {
                    self.tab_bar_scroll_offset = local_start.max(0.);
                }
                if local_end > self.tab_bar_scroll_offset + viewport_width {
                    self.tab_bar_scroll_offset = (local_end - viewport_width).max(0.);
                }
            }
            self.tab_bar_follow_active = false;
        }

        self.tab_bar_scroll_offset = self.tab_bar_scroll_offset.clamp(0., max_scroll);
        self.tab_bar_scroll_max = max_scroll;
        self.tab_bar_strip_clip = Some(euclid::rect(
            viewport_left,
            strip_y,
            viewport_width,
            strip_h,
        ));

        if self.tab_bar_scroll_offset != 0. {
            strip.translate(euclid::vec2(-self.tab_bar_scroll_offset, 0.));
        }
    }

    fn tab_bar_drag_gap_px(&self) -> f32 {
        self.fonts
            .title_font()
            .ok()
            .map(|font| {
                RenderMetrics::with_font_metrics(&font.metrics()).cell_size.width as f32 * 4.0
            })
            .unwrap_or(48.)
            .max(32.)
    }
}

fn is_scrolling_tab_strip(element: &ComputedElement) -> bool {
    if matches!(
        element.item_type,
        Some(UIItemType::TabBar(TabBarItem::Tab { .. })) | Some(UIItemType::CloseTab(_))
    ) {
        return false;
    }
    contains_item(element, |item| {
        matches!(item, UIItemType::TabBar(TabBarItem::Tab { .. }))
    })
}

fn scrolling_tab_strip_index(computed: &ComputedElement) -> Option<usize> {
    match &computed.content {
        ComputedElementContent::Children(kids) => kids.iter().position(is_scrolling_tab_strip),
        _ => None,
    }
}

fn contains_item(element: &ComputedElement, pred: impl Fn(&UIItemType) -> bool + Copy) -> bool {
    if let Some(item_type) = &element.item_type {
        if pred(item_type) {
            return true;
        }
    }
    match &element.content {
        ComputedElementContent::Children(kids) => kids.iter().any(|kid| contains_item(kid, pred)),
        _ => false,
    }
}

fn new_tab_button_index(computed: &ComputedElement) -> Option<usize> {
    match &computed.content {
        ComputedElementContent::Children(kids) => kids.iter().position(|kid| {
            contains_item(kid, |item| {
                matches!(item, UIItemType::TabBar(TabBarItem::NewTabButton))
            })
        }),
        _ => None,
    }
}

fn new_tab_visual_width(element: &ComputedElement) -> f32 {
    fn find(ele: &ComputedElement) -> Option<f32> {
        if matches!(
            ele.item_type,
            Some(UIItemType::TabBar(TabBarItem::NewTabButton))
        ) {
            return Some(ele.bounds.width());
        }
        match &ele.content {
            ComputedElementContent::Children(kids) => kids.iter().find_map(find),
            _ => None,
        }
    }
    find(element).unwrap_or_else(|| element.bounds.width())
}

fn find_right_chrome_left(computed: &ComputedElement) -> Option<f32> {
    match &computed.content {
        ComputedElementContent::Children(kids) => kids
            .iter()
            .filter(|kid| {
                contains_item(kid, |item| {
                    matches!(item, UIItemType::TabBar(TabBarItem::WindowButton(_)))
                })
            })
            .map(|kid| kid.bounds.min_x())
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)),
        _ => None,
    }
}

fn find_active_tab_bounds(element: &ComputedElement) -> Option<RectF> {
    if let Some(UIItemType::TabBar(TabBarItem::Tab { active: true, .. })) = &element.item_type {
        return Some(element.bounds);
    }
    match &element.content {
        ComputedElementContent::Children(kids) => kids.iter().find_map(find_active_tab_bounds),
        _ => None,
    }
}

fn clip_tab_strip_ui_items(items: Vec<UIItem>, clip: RectF) -> Vec<UIItem> {
    items
        .into_iter()
        .filter_map(|item| {
            let should_clip = matches!(
                item.item_type,
                UIItemType::TabBar(TabBarItem::Tab { .. }) | UIItemType::CloseTab(_)
            );
            if !should_clip {
                return Some(item);
            }
            let rect = euclid::rect(
                item.x as f32,
                item.y as f32,
                item.width as f32,
                item.height as f32,
            );
            let hit = rect.intersection(&clip)?;
            if hit.width() < 1. || hit.height() < 1. {
                return None;
            }
            Some(UIItem {
                x: hit.min_x().round().max(0.) as usize,
                y: hit.min_y().round().max(0.) as usize,
                width: hit.width().round().max(0.) as usize,
                height: hit.height().round().max(0.) as usize,
                item_type: item.item_type,
            })
        })
        .collect()
}

fn make_x_button(
    font: &Rc<LoadedFont>,
    metrics: &RenderMetrics,
    colors: &TabBarColors,
    tab_idx: usize,
    active: bool,
) -> Element {
    Element::new(
        &font,
        ElementContent::Poly {
            line_width: metrics.underline_height.max(2),
            poly: SizedPoly {
                poly: X_BUTTON,
                width: Dimension::Pixels(metrics.cell_size.height as f32 / 2.),
                height: Dimension::Pixels(metrics.cell_size.height as f32 / 2.),
            },
        },
    )
    // Ensure that we draw our background over the
    // top of the rest of the tab contents
    .zindex(1)
    .vertical_align(VerticalAlign::Middle)
    .float(Float::Right)
    .item_type(UIItemType::CloseTab(tab_idx))
    .hover_colors({
        let inactive_tab_hover = colors.inactive_tab_hover();
        let active_tab = colors.active_tab();

        Some(ElementColors {
            border: BorderColor::default(),
            bg: (if active {
                inactive_tab_hover.bg_color
            } else {
                active_tab.bg_color
            })
            .to_linear()
            .into(),
            text: (if active {
                inactive_tab_hover.fg_color
            } else {
                active_tab.fg_color
            })
            .to_linear()
            .into(),
        })
    })
    .padding(BoxDimension {
        left: Dimension::Cells(0.25),
        right: Dimension::Cells(0.25),
        top: Dimension::Cells(0.25),
        bottom: Dimension::Cells(0.25),
    })
    .margin(BoxDimension {
        left: Dimension::Cells(0.5),
        right: Dimension::Cells(0.),
        top: Dimension::Cells(0.),
        bottom: Dimension::Cells(0.),
    })
}
