//! Sibling-window confirm/prompt dialogs. POC replica of termwiz overlays
//! (`wezterm-gui` `overlay/confirm.rs` and `overlay/prompt.rs`). No mux.

use std::rc::Rc;

use gpui::*;
use gpui_component::{
    button::{Button, ButtonVariant, ButtonVariants as _},
    dialog::{DialogAction, DialogButtonProps, DialogClose, DialogFooter},
    input::{Input, InputState},
    StyledExt, WindowExt,
};

/// Yes/No confirm using gpui-component `AlertDialog`.
///
/// `on_ok` runs only if the user confirms. Cancel / Escape / close do nothing.
pub fn open_confirm(
    window: &mut Window,
    cx: &mut App,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    ok_text: impl Into<SharedString>,
    danger: bool,
    on_ok: impl Fn(&mut Window, &mut App) + 'static,
) {
    let title = title.into();
    let description = description.into();
    let ok_text = ok_text.into();
    let on_ok = Rc::new(on_ok);
    window.open_alert_dialog(cx, move |alert, _, _| {
        let on_ok = on_ok.clone();
        let mut props = DialogButtonProps::default()
            .ok_text(ok_text.clone())
            .show_cancel(true);
        if danger {
            props = props.ok_variant(ButtonVariant::Danger);
        }
        alert
            .title(title.clone())
            .description(description.clone())
            .button_props(props)
            .on_ok(move |_, window, cx| {
                on_ok(window, cx);
                true
            })
    });
}

/// Single-line prompt using gpui-component `Dialog` + `Input`.
///
/// `on_submit` runs with the trimmed field when the user confirms. Empty input
/// still submits (caller decides). Cancel / Escape do nothing.
pub fn open_line_prompt(
    window: &mut Window,
    cx: &mut App,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    placeholder: impl Into<SharedString>,
    initial: impl Into<SharedString>,
    on_submit: impl Fn(String, &mut Window, &mut App) + 'static,
) {
    let title = title.into();
    let description = description.into();
    let placeholder = placeholder.into();
    let initial = initial.into();
    let on_submit = Rc::new(on_submit);

    let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
    input.update(cx, |state, cx| {
        state.set_value(initial, window, cx);
        state.focus(window, cx);
    });

    window.open_dialog(cx, move |dialog, _, _| {
        let input = input.clone();
        let on_submit = on_submit.clone();
        dialog
            .title(title.clone())
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .child(description.clone())
                    .child(Input::new(&input)),
            )
            .on_ok(move |_, window, cx| {
                let value = input.read(cx).value().to_string();
                on_submit(value, window, cx);
                true
            })
            .footer(
                DialogFooter::new()
                    .child(
                        DialogClose::new()
                            .child(Button::new("cancel").outline().label("Cancel")),
                    )
                    .child(
                        DialogAction::new()
                            .child(Button::new("ok").primary().label("OK")),
                    ),
            )
    });
}
