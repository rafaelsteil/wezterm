//! Sibling GPUI binary. Does not start WezTerm's `window` event loop.

use gpui::*;
use gpui_component::*;
use wezterm_gpui::{bind_keys, AppShell, HelloWorld};

fn window_options(cx: &App) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(980.), px(640.)),
            cx,
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some("WezTerm GPUI".into()),
            ..TitleBar::title_bar_options()
        }),
        ..Default::default()
    }
}

fn hello_window_options() -> WindowOptions {
    WindowOptions {
        titlebar: Some(TitlebarOptions {
            title: Some("WezTerm GPUI — Hello".into()),
            appears_transparent: false,
            traffic_light_position: None,
        }),
        ..Default::default()
    }
}

fn main() {
    let hello = std::env::args().any(|a| a == "--hello");

    gpui_platform::application().run(move |cx| {
        gpui_component::init(cx);
        bind_keys(cx);

        if hello {
            cx.spawn(async move |cx| {
                cx.open_window(hello_window_options(), |window, cx| {
                    let view = cx.new(|_| HelloWorld);
                    cx.new(|cx| Root::new(view, window, cx).bg(cx.theme().background))
                })
                .expect("Failed to open window");
            })
            .detach();
        } else {
            let opts = window_options(cx);
            cx.spawn(async move |cx| {
                cx.open_window(opts, |window, cx| {
                    let view = cx.new(|cx| AppShell::new(window, cx));
                    cx.new(|cx| Root::new(view, window, cx).bg(cx.theme().background))
                })
                .expect("Failed to open window");
            })
            .detach();
        }
    });
}
