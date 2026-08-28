//! Sibling-window confirm/prompt dialogs. POC replica of termwiz overlays
//! (`wezterm-gui` `overlay/confirm.rs` and `overlay/prompt.rs`). No mux.

use std::rc::Rc;

use gpui::*;
use gpui_component::{
    StyledExt, WindowExt,
    button::{Button, ButtonVariant, ButtonVariants as _},
    dialog::{DialogAction, DialogButtonProps, DialogClose, DialogFooter},
    input::{Input, InputState},
};

/// Yes/No confirm using gpui-component `AlertDialog`.
///
/// `on_ok` runs only if the user confirms. `on_close` runs after OK **or**
/// Cancel / Escape. Wired through AlertDialog `on_ok`/`on_cancel` (not
/// `on_close`): `build_surface` copies `button_props` and would drop a
/// base-level `on_close`. Needed so AppShell is focused again (032 leftover:
/// dialog restore targets the tab Close button, so typing dies).
pub fn open_confirm(
    window: &mut Window,
    cx: &mut App,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    ok_text: impl Into<SharedString>,
    danger: bool,
    on_ok: impl Fn(&mut Window, &mut App) + 'static,
    on_close: impl Fn(&mut Window, &mut App) + 'static,
) {
    let title = title.into();
    let description = description.into();
    let ok_text = ok_text.into();
    let on_ok = Rc::new(on_ok);
    let on_close = Rc::new(on_close);
    window.open_alert_dialog(cx, move |alert, _, _| {
        let on_ok = on_ok.clone();
        let on_close_ok = on_close.clone();
        let on_close_cancel = on_close.clone();
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
                on_close_ok(window, cx);
                true
            })
            .on_cancel(move |_, window, cx| {
                on_close_cancel(window, cx);
                true
            })
    });
}

/// Single-line prompt using gpui-component `Dialog` + `Input`.
///
/// `on_submit` runs with the trimmed field when the user confirms. Empty input
/// still submits (caller decides). `on_close` runs after OK or Cancel.
pub fn open_line_prompt(
    window: &mut Window,
    cx: &mut App,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    placeholder: impl Into<SharedString>,
    initial: impl Into<SharedString>,
    on_submit: impl Fn(String, &mut Window, &mut App) + 'static,
    on_close: impl Fn(&mut Window, &mut App) + 'static,
) {
    let title = title.into();
    let description = description.into();
    let placeholder = placeholder.into();
    let initial = initial.into();
    let on_submit = Rc::new(on_submit);
    let on_close = Rc::new(on_close);

    let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
    input.update(cx, |state, cx| {
        state.set_value(initial, window, cx);
        state.focus(window, cx);
    });

    window.open_dialog(cx, move |dialog, _, _| {
        let input = input.clone();
        let on_submit = on_submit.clone();
        let on_close = on_close.clone();
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
            .on_close(move |_, window, cx| {
                on_close(window, cx);
            })
            .footer(
                DialogFooter::new()
                    .child(
                        DialogClose::new().child(Button::new("cancel").outline().label("Cancel")),
                    )
                    .child(DialogAction::new().child(Button::new("ok").primary().label("OK"))),
            )
    });
}
