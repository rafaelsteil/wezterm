# Mount Root dialog / notification layers (038)

User 2026-08-27 after **037**: Ctrl+Q to quit showed **no confirmation dialog**. Nothing visible happened. Combined with the original report (typing dead after Ctrl+Q / tab X, including in new tabs).

## Cause

`window.open_alert_dialog` **does** push an `AlertDialog` onto `Root.active_dialogs` and focuses it. gpui-component **Root does not paint that list**. Official docs / story / `window_selection.rs` comment: the first-level view (Root → AppShell) must call:

- `Root::render_dialog_layer`
- `Root::render_sheet_layer`
- `Root::render_notification_layer`

AppShell never did. Result: invisible modal (`has_active_dialog` true, focus trap on a dialog that is not on screen). Keys skipped in `on_term_key`. Plus still clicks. Same for tab X.

037 focus-restore is still needed **after** a visible dialog is dismissed. This slice makes the dialog exist on screen.

## In this slice

Mount the three Root overlay layers in `AppShell::render` (dialog, sheet, notification), above the FPS HUD.

## Out

- 026 / 031
- Confirm-less Ctrl+Q / X

Record: `docs/decisions/038-mount-root-dialog-layer.json`.
