//! Sibling GPUI binary. Does not start WezTerm's `window` event loop.

use gpui::*;
use gpui_component::*;
use wezterm_gpui::{app_window_options, bind_keys, AppShell, HelloWorld};

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

    gpui_platform::application()
        .with_assets(gpui_component_assets::Assets)
        .run(move |cx| {
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
                let opts = app_window_options(cx);
                cx.spawn(async move |cx| {
                    cx.open_window(opts, |window, cx| {
                        let view = cx.new(|cx| AppShell::new(window, cx));
                        let root = cx.new(|cx| {
                            Root::new(view.clone(), window, cx).bg(cx.theme().background)
                        });
                        view.update(cx, |shell, cx| shell.focus_terminal(window, cx));
                        root
                    })
                    .expect("Failed to open window");
                })
                .detach();
            }
        });
}
