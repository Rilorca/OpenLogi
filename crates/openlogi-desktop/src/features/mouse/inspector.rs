//! Fixed binding inspector for the Buttons workspace.

use std::collections::BTreeMap;
use std::rc::Rc;

use gpui::{
    Context, Entity, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    Role, StatefulInteractiveElement as _, Styled, div, prelude::FluentBuilder as _, px, rgb, svg,
};
use gpui_base::Button as BaseButton;
use gpui_component::{
    Disableable as _, Icon, IconName, Selectable as _, Sizable as _, button::Button, h_flex,
    input::InputState, scroll::ScrollableElement as _, v_flex,
};
use openlogi_core::binding::{
    Action, ButtonId, GestureDirection, KeyCombo, KeyboardUsage, ModifierKey, default_binding,
};

use super::hotspots::MouseControlId;
use super::thumbwheel::ThumbwheelPreset;
use super::view::MouseModelView;
use crate::features::binding_editor::{
    GESTURE_BUTTON_ICON, PickFn, action_icon_path, action_rows_matching, editor_section,
    gesture_direction_icon,
};
use crate::state::AppState;
use crate::ui::action::localized_action_label;
use crate::ui::components::{MenuRow, control_button, control_input};
use crate::ui::theme::{self, ACCENT_BLUE, Palette, Typography as _};

pub(super) const INSPECTOR_W: f32 = 328.;

#[derive(Clone, Copy)]
pub(super) struct BindingInspectorData<'a> {
    pub selected: Option<MouseControlId>,
    pub gesture_direction: Option<GestureDirection>,
    pub action_picker_open: bool,
    pub bindings: &'a BTreeMap<ButtonId, Action>,
    pub gesture_maps: &'a BTreeMap<ButtonId, BTreeMap<GestureDirection, Action>>,
    pub dpi_gestures: bool,
    pub editing_app: Option<&'a str>,
    pub overridden: Option<&'a BTreeMap<ButtonId, Action>>,
    /// Focus target for the "Record shortcut…" row — see
    /// [`MouseModelView::is_recording_shortcut`].
    pub recorder_focus: &'a FocusHandle,
    pub recording_shortcut: bool,
}

#[derive(Clone, Copy)]
struct ActionPickerContext<'a> {
    open: bool,
    search: &'a Entity<InputState>,
    view: &'a Entity<MouseModelView>,
    recorder_focus: &'a FocusHandle,
    recording: bool,
}

pub(super) fn binding_inspector(
    data: BindingInspectorData<'_>,
    action_search: &Entity<InputState>,
    view: &Entity<MouseModelView>,
    cx: &Context<MouseModelView>,
) -> gpui::Div {
    let pal = theme::palette(cx);
    let picker = ActionPickerContext {
        open: data.action_picker_open,
        search: action_search,
        view,
        recorder_focus: data.recorder_focus,
        recording: data.recording_shortcut,
    };
    let body = match data.selected {
        None => empty_inspector(
            data.editing_app,
            data.overridden.map_or(0, BTreeMap::len),
            pal,
        ),
        Some(MouseControlId::ThumbwheelRotation) => thumbwheel_inspector(
            data.bindings,
            data.editing_app,
            data.overridden,
            picker,
            pal,
        ),
        Some(MouseControlId::Button(button)) => button_inspector(button, &data, picker, pal, cx),
    };

    v_flex()
        .w(px(INSPECTOR_W))
        .h_full()
        .min_h_0()
        .flex_shrink_0()
        .border_l_1()
        .border_color(pal.border)
        .bg(pal.panel)
        .child(
            div()
                .id("button-inspector-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scrollbar()
                .p_4()
                .child(body),
        )
}

fn empty_inspector(app: Option<&str>, override_count: usize, pal: Palette) -> gpui::Div {
    let summary = match (app, override_count) {
        (Some(app), 0) => tr!(
            "profiles.app_profile_no_overrides",
            app => app.to_string()
        ),
        (Some(app), 1) => tr!(
            "profiles.app_profile_single_override",
            app => app.to_string()
        ),
        (Some(app), count) => tr!(
            "profiles.app_profile_override_count",
            app => app.to_string(),
            count => count.to_string()
        ),
        (None, _) => tr!("profiles.select_device_button_description"),
    };
    v_flex()
        .gap_3()
        .child(inspector_heading(
            tr!("actions.button_inspector"),
            None,
            pal,
        ))
        .child(div().text_body().text_color(pal.text_muted).child(summary))
}

fn button_inspector(
    button: ButtonId,
    data: &BindingInspectorData<'_>,
    picker: ActionPickerContext<'_>,
    pal: Palette,
    cx: &Context<MouseModelView>,
) -> gpui::Div {
    let gesture_map = data.gesture_maps.get(&button);
    let overridden = data
        .overridden
        .is_some_and(|overrides| overrides.contains_key(&button));
    if data.editing_app.is_none()
        && let Some(gesture_map) = gesture_map
    {
        return gesture_inspector(button, gesture_map, data.gesture_direction, picker, pal, cx);
    }
    if let Some(app) = data.editing_app
        && !overridden
        && gesture_map.is_some()
    {
        return inherited_gesture_inspector(button, app, picker, pal, cx);
    }

    let action = data
        .bindings
        .get(&button)
        .cloned()
        .unwrap_or_else(|| default_binding(button));
    let status = match (
        data.editing_app,
        overridden,
        action == default_binding(button),
    ) {
        (Some(app), true, _) => tr!("actions.overridden_in_app", app => app.to_string()),
        (Some(_), false, _) => tr!("profiles.inherited_from_default"),
        (None, _, true) => tr!("pointer.device_default"),
        (None, _, false) => tr!("profiles.customized"),
    };
    let observer = picker.view.clone();
    let on_pick: PickFn = Rc::new(move |action, _window, cx| {
        AppState::apply(cx, |state| state.commit_binding(button, action));
        observer.update(cx, |view, cx| {
            view.close_action_picker();
            cx.notify();
        });
    });

    v_flex()
        .gap_3()
        .child(inspector_heading(
            tr!(button.translation_key()),
            Some(status),
            pal,
        ))
        .child(current_action_card(&action, picker, pal))
        .when(overridden, |panel| {
            let observer = picker.view.clone();
            panel.child(
                control_button("inspector-use-default")
                    .w_full()
                    .icon(IconName::Undo)
                    .label(tr!("profiles.use_the_default_profile"))
                    .on_click(move |_, _, cx| {
                        AppState::apply(cx, |state| state.clear_app_binding(button));
                        observer.update(cx, |view, cx| {
                            view.close_action_picker();
                            cx.notify();
                        });
                    }),
            )
        })
        .when(can_enable_gestures(button, data.editing_app), |panel| {
            let observer = picker.view.clone();
            let unavailable = button == ButtonId::DpiToggle && !data.dpi_gestures;
            panel
                .child(
                    control_button("inspector-use-gestures")
                        .w_full()
                        .icon(Icon::empty().path(GESTURE_BUTTON_ICON))
                        .label(tr!("actions.use_gestures"))
                        .disabled(unavailable)
                        .on_click(move |_, _, cx| {
                            AppState::apply(cx, |state| state.commit_gesture_mode(button, true));
                            observer.update(cx, |view, cx| {
                                view.set_gesture_selected_dir(Some(GestureDirection::Click));
                                cx.notify();
                            });
                        }),
                )
                .when(unavailable, |panel| {
                    panel.child(
                        div()
                            .text_body()
                            .text_color(pal.text_muted)
                            .child(tr!("actions.dpi_gestures_unavailable")),
                    )
                })
        })
        .when(picker.open, |panel| {
            panel.child(action_library(
                "inspector-action",
                Some(&action),
                &on_pick,
                picker,
                pal,
                cx,
            ))
        })
}

fn inherited_gesture_inspector(
    button: ButtonId,
    app: &str,
    picker: ActionPickerContext<'_>,
    pal: Palette,
    cx: &Context<MouseModelView>,
) -> gpui::Div {
    let observer = picker.view.clone();
    let on_pick: PickFn = Rc::new(move |action, _window, cx| {
        AppState::apply(cx, |state| state.commit_binding(button, action));
        observer.update(cx, |view, cx| {
            view.close_action_picker();
            cx.notify();
        });
    });
    let edit_default = picker.view.clone();
    v_flex()
        .gap_3()
        .child(inspector_heading(
            tr!(button.translation_key()),
            Some(tr!("profiles.inherited_from_default")),
            pal,
        ))
        .child(gesture_summary_card(picker, pal))
        .child(div().text_caption().text_color(pal.text_muted).child(tr!(
            "actions.app_profile_action_replaces_gestures",
            app => app.to_string()
        )))
        .child(
            Button::new("inspector-edit-default-gestures")
                .small()
                .w_full()
                .label(tr!("actions.edit_default_gestures"))
                .on_click(move |_, _, cx| {
                    AppState::apply(cx, |state| state.set_editing_app(None));
                    edit_default.update(cx, |view, cx| {
                        view.set_gesture_selected_dir(Some(GestureDirection::Click));
                        cx.notify();
                    });
                }),
        )
        .when(picker.open, |panel| {
            panel.child(action_library(
                "inspector-gesture-override",
                None,
                &on_pick,
                picker,
                pal,
                cx,
            ))
        })
}

fn gesture_inspector(
    button: ButtonId,
    gesture_map: &BTreeMap<GestureDirection, Action>,
    selected_direction: Option<GestureDirection>,
    picker: ActionPickerContext<'_>,
    pal: Palette,
    cx: &Context<MouseModelView>,
) -> gpui::Div {
    let direction = selected_direction.unwrap_or(GestureDirection::Click);
    let current = gesture_action(gesture_map, button, direction);
    let observer = picker.view.clone();
    let on_pick: PickFn = Rc::new(move |action, _window, cx| {
        AppState::apply(cx, |state| {
            state.commit_gesture_binding(button, direction, action)
        });
        observer.update(cx, |view, cx| {
            view.close_action_picker();
            cx.notify();
        });
    });
    let turn_off = picker.view.clone();

    v_flex()
        .gap_3()
        .child(inspector_heading(
            tr!(button.translation_key()),
            Some(tr!("actions.five_directions")),
            pal,
        ))
        .child(gesture_directions(
            direction,
            gesture_map,
            button,
            picker.view,
            pal,
        ))
        .child(current_action_card(&current, picker, pal))
        .child(
            control_button("inspector-single-action")
                .w_full()
                .label(tr!("actions.use_a_single_action"))
                .on_click(move |_, _, cx| {
                    AppState::apply(cx, |state| state.commit_gesture_mode(button, false));
                    turn_off.update(cx, |view, cx| {
                        view.set_gesture_selected_dir(None);
                        cx.notify();
                    });
                }),
        )
        .when(picker.open, |panel| {
            panel.child(action_library(
                "inspector-gesture-action",
                Some(&current),
                &on_pick,
                picker,
                pal,
                cx,
            ))
        })
}

fn gesture_directions(
    active: GestureDirection,
    gesture_map: &BTreeMap<GestureDirection, Action>,
    button: ButtonId,
    view: &Entity<MouseModelView>,
    pal: Palette,
) -> impl IntoElement {
    v_flex()
        .gap_1()
        .child(editor_section(tr!("actions.direction"), pal))
        .children(
            GestureDirection::ALL
                .into_iter()
                .enumerate()
                .map(|(index, direction)| {
                    let selected = direction == active;
                    let action = gesture_action(gesture_map, button, direction);
                    let view = view.clone();
                    MenuRow::new(("inspector-direction", index))
                        .selected(selected)
                        .role(Role::Button)
                        .child(
                            h_flex()
                                .min_w_0()
                                .gap_2()
                                // `.size_4()` is not decoration: a bare `Icon`
                                // falls through to the current font size, which
                                // would leave these a step under the 16px leading
                                // column the action rows below use.
                                .child(gesture_direction_icon(direction).size_4())
                                .child(
                                    v_flex()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .text_body()
                                                .child(tr!(direction.translation_key())),
                                        )
                                        .child(
                                            div()
                                                .truncate()
                                                .text_caption()
                                                .text_color(pal.text_muted)
                                                .child(localized_action_label(&action)),
                                        ),
                                ),
                        )
                        .when(selected, |row| {
                            row.child(
                                Icon::new(IconName::Check)
                                    .size_3()
                                    .text_color(rgb(ACCENT_BLUE)),
                            )
                        })
                        .on_click(move |_, _, cx| {
                            view.update(cx, |view, cx| {
                                view.set_gesture_selected_dir(Some(direction));
                                cx.notify();
                            });
                        })
                }),
        )
}

/// Whether the default-profile inspector may promote `button` into gesture
/// mode. Per-app bindings are single-action overrides, so they cannot carry a
/// direction map.
fn can_enable_gestures(button: ButtonId, editing_app: Option<&str>) -> bool {
    editing_app.is_none() && button.supports_gesture_mode()
}

fn thumbwheel_inspector(
    bindings: &BTreeMap<ButtonId, Action>,
    editing_app: Option<&str>,
    overridden: Option<&BTreeMap<ButtonId, Action>>,
    picker: ActionPickerContext<'_>,
    pal: Palette,
) -> gpui::Div {
    let backward = bindings
        .get(&ButtonId::ThumbwheelScrollDown)
        .cloned()
        .unwrap_or_else(|| default_binding(ButtonId::ThumbwheelScrollDown));
    let forward = bindings
        .get(&ButtonId::ThumbwheelScrollUp)
        .cloned()
        .unwrap_or_else(|| default_binding(ButtonId::ThumbwheelScrollUp));
    let current = ThumbwheelPreset::recognize(&backward, &forward);
    let is_overridden = overridden.is_some_and(|overrides| {
        overrides.contains_key(&ButtonId::ThumbwheelScrollDown)
            || overrides.contains_key(&ButtonId::ThumbwheelScrollUp)
    });
    let status = match (editing_app, is_overridden) {
        (Some(app), true) => tr!("actions.overridden_in_app", app => app.to_string()),
        (Some(_), false) => tr!("profiles.inherited_from_default"),
        (None, _) => tr!("profiles.default_profile"),
    };
    let current_label = current.map_or_else(
        || tr!("common.custom"),
        |preset| tr!(preset.translation_key()),
    );
    let current_icon = current.map_or("action-icons/chevrons-right.svg", ThumbwheelPreset::icon);
    let observer = picker.view.clone();

    v_flex()
        .gap_3()
        .child(inspector_heading(
            tr!("pointer.thumb_wheel"),
            Some(status),
            pal,
        ))
        .child(selection_card(
            "inspector-current-thumbwheel-preset",
            tr!("common.preset"),
            current_icon,
            current_label,
            picker,
            pal,
        ))
        .when(picker.open, |panel| {
            panel.child(
                v_flex()
                    .gap_1()
                    .child(editor_section(tr!("common.preset"), pal))
                    .children(ThumbwheelPreset::ALL.into_iter().enumerate().map(
                        |(index, preset)| {
                            let selected = current == Some(preset);
                            let observer = observer.clone();
                            MenuRow::new(("inspector-thumbwheel", index))
                                .selected(selected)
                                .role(Role::Button)
                                .child(
                                    h_flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            svg()
                                                .path(preset.icon())
                                                .size_4()
                                                .text_color(pal.text_muted),
                                        )
                                        .child(div().child(tr!(preset.translation_key()))),
                                )
                                .when(selected, |row| {
                                    row.child(
                                        Icon::new(IconName::Check)
                                            .size_3()
                                            .text_color(rgb(ACCENT_BLUE)),
                                    )
                                })
                                .on_click(move |_, _, cx| {
                                    AppState::apply(cx, |state| {
                                        state.commit_thumbwheel_preset(preset)
                                    });
                                    observer.update(cx, |view, cx| {
                                        view.close_action_picker();
                                        cx.notify();
                                    });
                                })
                        },
                    )),
            )
        })
        .when(is_overridden, |panel| {
            let observer = picker.view.clone();
            panel.child(
                Button::new("inspector-thumbwheel-use-default")
                    .small()
                    .w_full()
                    .icon(IconName::Undo)
                    .label(tr!("profiles.use_the_default_profile"))
                    .on_click(move |_, _, cx| {
                        AppState::apply(cx, AppState::clear_app_thumbwheel);
                        observer.update(cx, |view, cx| {
                            view.close_action_picker();
                            cx.notify();
                        });
                    }),
            )
        })
}

fn inspector_heading(
    title: gpui::SharedString,
    status: Option<gpui::SharedString>,
    pal: Palette,
) -> impl IntoElement {
    v_flex()
        .gap_1()
        .child(div().text_heading().child(title))
        .children(status.map(|status| {
            div()
                .text_caption()
                .text_color(pal.text_muted)
                .child(status)
        }))
}

fn current_action_card(
    action: &Action,
    picker: ActionPickerContext<'_>,
    pal: Palette,
) -> impl IntoElement {
    selection_card(
        "inspector-current-action",
        tr!("actions.current_action"),
        action_icon_path(action),
        localized_action_label(action),
        picker,
        pal,
    )
}

fn gesture_summary_card(picker: ActionPickerContext<'_>, pal: Palette) -> impl IntoElement {
    selection_card(
        "inspector-current-gesture-summary",
        tr!("actions.current_action"),
        GESTURE_BUTTON_ICON,
        tr!("actions.five_directions"),
        picker,
        pal,
    )
}

fn selection_card(
    id: &'static str,
    caption: gpui::SharedString,
    icon: &'static str,
    value: gpui::SharedString,
    picker: ActionPickerContext<'_>,
    pal: Palette,
) -> impl IntoElement {
    let toggle = picker.view.clone();
    let search = picker.search.clone();
    let opening = !picker.open;
    let accessible_label = value.clone();
    BaseButton::new(id)
        .accessibility_label(accessible_label)
        .aria_expanded(picker.open)
        .flex()
        .flex_col()
        .items_stretch()
        .gap_2()
        .rounded(pal.control_radius)
        .border_1()
        .border_color(pal.border)
        .bg(pal.control)
        .p_3()
        .cursor_pointer()
        .hover(move |card| card.bg(pal.control_hover))
        .focus_visible(move |card| card.bg(pal.control_hover).border_color(rgb(ACCENT_BLUE)))
        .child(
            div()
                .text_caption()
                .text_color(pal.text_muted)
                .child(caption),
        )
        .child(
            h_flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    h_flex()
                        .min_w_0()
                        .items_center()
                        .gap_2()
                        .child(
                            svg()
                                .path(icon)
                                .size_4()
                                .flex_none()
                                .text_color(pal.text_muted),
                        )
                        .child(div().min_w_0().truncate().text_body().child(value)),
                )
                .child(
                    svg()
                        .path(if picker.open {
                            "action-icons/chevrons-up.svg"
                        } else {
                            "action-icons/chevrons-down.svg"
                        })
                        .size_3()
                        .flex_none()
                        .text_color(pal.text_muted),
                ),
        )
        .on_click(move |_, window, cx| {
            if opening {
                search.update(cx, |search, cx| search.set_value("", window, cx));
            }
            toggle.update(cx, |view, cx| {
                view.toggle_action_picker();
                cx.notify();
            });
        })
}

fn action_library(
    id_prefix: &'static str,
    current: Option<&Action>,
    on_pick: &PickFn,
    picker: ActionPickerContext<'_>,
    pal: Palette,
    cx: &Context<MouseModelView>,
) -> impl IntoElement {
    let query = picker.search.read(cx).value();
    let rows = action_rows_matching(id_prefix, current, &query, on_pick, pal);
    v_flex()
        .gap_2()
        .pt_1()
        .child(editor_section(tr!("actions.actions"), pal))
        .child(record_shortcut_row(id_prefix, on_pick, picker, pal))
        .child(control_input(picker.search).cleanable(true))
        .child(
            v_flex()
                .gap_0p5()
                .when(rows.is_empty(), |list| {
                    list.child(
                        div()
                            .py_3()
                            .text_body()
                            .text_color(pal.text_muted)
                            .child(tr!("actions.no_actions_found")),
                    )
                })
                .children(rows),
        )
}

/// Translate one captured keystroke into a chord.
///
/// `None` for a keystroke that is not a bindable chord: a bare modifier press
/// (the user is still assembling the combination, so `keystroke.key` is
/// `"shift"`, `"ctrl"`, …), or a key custom shortcuts don't model. Both cases
/// mean "keep waiting", not "commit something wrong".
fn combo_from_keystroke(keystroke: &gpui::Keystroke) -> Option<KeyCombo> {
    let key = KeyboardUsage::from_name(&keystroke.key).ok()?;
    let modifiers = &keystroke.modifiers;
    Some(
        KeyCombo::new(key)
            // gpui calls this `platform`: Command on macOS, Win on Windows,
            // Super on Linux — exactly what `KeyCombo` models as "command".
            .with_modifier_if(modifiers.platform, ModifierKey::Command)
            .with_modifier_if(modifiers.shift, ModifierKey::Shift)
            .with_modifier_if(modifiers.control, ModifierKey::Control)
            .with_modifier_if(modifiers.alt, ModifierKey::Option),
    )
}

/// The "Record shortcut…" row: click it, then press the chord you want bound.
///
/// This is the escape hatch [`Action::CustomShortcut`] exists for — any chord
/// the user can physically press, not a fixed list. Escape cancels; a bare
/// modifier press is ignored so the user can hold Ctrl+Shift before landing on
/// the final key. Picking a chord goes through the same `on_pick` every other
/// row in the library uses, so it commits and closes the picker identically.
fn record_shortcut_row(
    id_prefix: &'static str,
    on_pick: &PickFn,
    picker: ActionPickerContext<'_>,
    pal: Palette,
) -> impl IntoElement {
    let recording = picker.recording;
    let label = if recording {
        tr!("Press a shortcut…")
    } else {
        tr!("Record shortcut…")
    };
    let view_click = picker.view.clone();
    let focus_click = picker.recorder_focus.clone();
    let view_key = picker.view.clone();
    let on_pick_key = on_pick.clone();

    MenuRow::new(format!("{id_prefix}-record-shortcut"))
        .selected(recording)
        .role(Role::MenuItem)
        .aria_label(label.clone())
        .track_focus(picker.recorder_focus)
        .child(
            h_flex()
                .items_center()
                .gap_2()
                .child(
                    svg()
                        .path("action-icons/keyboard.svg")
                        .size_4()
                        .flex_none()
                        .text_color(if recording {
                            rgb(ACCENT_BLUE).into()
                        } else {
                            pal.text_muted
                        }),
                )
                .child(div().child(label)),
        )
        .on_click(move |_event, window, cx| {
            view_click.update(cx, |view, cx| {
                view.start_recording_shortcut();
                cx.notify();
            });
            window.focus(&focus_click, cx);
        })
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            if !view_key.read(cx).is_recording_shortcut() {
                return;
            }
            // Escape cancels rather than binding Escape itself. Binding Escape
            // to a mouse button is possible via a chord (Shift+Escape); giving
            // the bare key to "cancel" is the convention every recorder uses.
            if event.keystroke.key == "escape" && !event.keystroke.modifiers.modified() {
                view_key.update(cx, |view, cx| {
                    view.stop_recording_shortcut();
                    cx.notify();
                });
                return;
            }
            let Some(combo) = combo_from_keystroke(&event.keystroke) else {
                // Bare modifier or unsupported key — keep waiting.
                return;
            };
            view_key.update(cx, |view, cx| {
                view.stop_recording_shortcut();
                cx.notify();
            });
            (on_pick_key)(Action::CustomShortcut(combo), window, cx);
        })
}

fn gesture_action(
    gesture_map: &BTreeMap<GestureDirection, Action>,
    button: ButtonId,
    direction: GestureDirection,
) -> Action {
    gesture_map.get(&direction).cloned().unwrap_or_else(|| {
        if direction == GestureDirection::Click {
            default_binding(button)
        } else {
            Action::None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_offers_gestures_for_every_supported_button() {
        let supported: Vec<_> = ButtonId::ALL
            .into_iter()
            .filter(|button| can_enable_gestures(*button, None))
            .collect();

        assert_eq!(
            supported,
            vec![
                ButtonId::Back,
                ButtonId::Forward,
                ButtonId::DpiToggle,
                ButtonId::GestureButton,
                ButtonId::HapticPanel,
            ]
        );
    }

    #[test]
    fn per_app_profile_does_not_offer_forward_gesture_mode() {
        assert!(!can_enable_gestures(
            ButtonId::Forward,
            Some("com.apple.Safari")
        ));
    }
}
