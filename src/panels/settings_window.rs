use gpui::*;
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex, v_flex, ActiveTheme as _, IconName, Root, Sizable as _, StyledExt as _, TitleBar,
};

actions!(settings, [CloseSettingsWindow]);

pub struct SettingsWindow {
    selected_tab: SettingsTab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    General,
    Appearance,
    Editor,
    Extensions,
}

impl SettingsWindow {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            selected_tab: SettingsTab::General,
        }
    }

    pub fn open(cx: &mut App) -> Task<anyhow::Result<WindowHandle<Root>>> {
        let window_size = size(px(800.0), px(600.0));
        let window_bounds = Bounds::centered(None, window_size, cx);

        cx.spawn(async move |cx| {
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(window_bounds)),
                #[cfg(not(target_os = "linux"))]
                titlebar: Some(gpui_component::TitleBar::title_bar_options()),
                window_min_size: Some(gpui::Size {
                    width: px(600.),
                    height: px(400.),
                }),
                #[cfg(target_os = "linux")]
                window_background: gpui::WindowBackgroundAppearance::Transparent,
                #[cfg(target_os = "linux")]
                window_decorations: Some(gpui::WindowDecorations::Client),
                kind: WindowKind::Normal,
                ..Default::default()
            };

            let window = cx.open_window(options, |window, cx| {
                let settings_view = cx.new(|cx| SettingsWindow::new(window, cx));
                cx.new(|cx| Root::new(settings_view, window, cx))
            })?;

            window
                .update(cx, |_, window, cx| {
                    window.set_window_title("Settings");
                    window.activate_window();
                    cx.on_release(|_, _| {
                        // Just close this window, don't quit the app
                    })
                    .detach();
                })
                .expect("failed to update window");

            Ok(window)
        })
    }

    fn render_title_bar(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        TitleBar::new()
            .child(
                div().flex().items_center().px_2().child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().tab_foreground)
                        .child("Settings"),
                ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_end()
                    .px_2()
                    .gap_2()
                    .child(
                        Button::new("close-settings")
                            .icon(IconName::Close)
                            .small()
                            .ghost()
                            .on_click(|_, _, cx| {
                                cx.dispatch_action(&CloseSettingsWindow);
                            }),
                    ),
            )
    }

    fn render_sidebar(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tabs = [
            (SettingsTab::General, "General", IconName::Settings),
            (SettingsTab::Appearance, "Appearance", IconName::Palette),
            (SettingsTab::Editor, "Editor", IconName::File),
            (SettingsTab::Extensions, "Extensions", IconName::Star),
        ];

        v_flex()
            .w(px(200.))
            .h_full()
            .bg(cx.theme().background)
            .border_r_1()
            .border_color(cx.theme().border)
            .p_2()
            .gap_1()
            .children(tabs.iter().map(|(tab, label, icon)| {
                let is_selected = *tab == self.selected_tab;
                let tab_copy = *tab;
                if is_selected {
                    Button::new(*label)
                        .label(*label)
                        .icon(icon.clone())
                        .w_full()
                        .justify_start()
                        .primary()
                        .on_click(cx.listener(move |this, _, _, _| {
                            this.selected_tab = tab_copy;
                        }))
                } else {
                    Button::new(*label)
                        .label(*label)
                        .icon(icon.clone())
                        .w_full()
                        .justify_start()
                        .ghost()
                        .on_click(cx.listener(move |this, _, _, _| {
                            this.selected_tab = tab_copy;
                        }))
                }
            }))
    }

    fn render_content(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .flex_1()
            .h_full()
            .bg(cx.theme().background)
            .p_4()
            .gap_4()
            .child(match self.selected_tab {
                SettingsTab::General => self.render_general_settings(cx).into_any_element(),
                SettingsTab::Appearance => self.render_appearance_settings(cx).into_any_element(),
                SettingsTab::Editor => self.render_editor_settings(cx).into_any_element(),
                SettingsTab::Extensions => self.render_extensions_settings(cx).into_any_element(),
            })
    }

    fn render_general_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .child(
                div()
                    .text_lg()
                    .font_semibold()
                    .text_color(cx.theme().foreground)
                    .child("General Settings"),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child("Language"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().foreground)
                            .child("English"),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child("Auto Save"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().foreground)
                            .child("After Delay"),
                    ),
            )
    }

    fn render_appearance_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .child(
                div()
                    .text_lg()
                    .font_semibold()
                    .text_color(cx.theme().foreground)
                    .child("Appearance Settings"),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child("Theme"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().foreground)
                            .child("Dark"),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child("Font Size"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().foreground)
                            .child("16px"),
                    ),
            )
    }

    fn render_editor_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .child(
                div()
                    .text_lg()
                    .font_semibold()
                    .text_color(cx.theme().foreground)
                    .child("Editor Settings"),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child("Tab Size"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().foreground)
                            .child("4 spaces"),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child("Line Numbers"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().foreground)
                            .child("Enabled"),
                    ),
            )
    }

    fn render_extensions_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .child(
                div()
                    .text_lg()
                    .font_semibold()
                    .text_color(cx.theme().foreground)
                    .child("Extensions Settings"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("No extensions installed"),
            )
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sheet_layer = Root::render_sheet_layer(window, cx);
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);

        div()
            .id("settings-window")
            .on_action(cx.listener(|_, _: &CloseSettingsWindow, window, _| {
                window.remove_window();
            }))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .child(self.render_title_bar(window, cx))
            .child(
                h_flex()
                    .flex_1()
                    .size_full()
                    .child(self.render_sidebar(window, cx))
                    .child(self.render_content(window, cx)),
            )
            .children(sheet_layer)
            .children(dialog_layer)
            .children(notification_layer)
    }
}
