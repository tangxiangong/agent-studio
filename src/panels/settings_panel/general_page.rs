use gpui::{App, Axis, Entity, ParentElement as _, SharedString, Styled};
use gpui_component::{
    ActiveTheme, IconName, Sizable, Size, Theme, ThemeMode,
    button::Button,
    group_box::GroupBoxVariant,
    h_flex,
    setting::{NumberFieldOptions, SettingField, SettingGroup, SettingItem, SettingPage},
};
use rust_i18n::t;

use super::panel::SettingsPanel;
use super::types::AppSettings;

impl SettingsPanel {
    pub fn general_page(&self, _view: &Entity<Self>, resettable: bool) -> SettingPage {
        let default_settings = AppSettings::default();

        SettingPage::new(t!("settings.general.title").to_string())
            .resettable(resettable)
            .default_open(true)
            .groups(vec![
                SettingGroup::new()
                    .title(t!("settings.general.group.appearance").to_string())
                    .items(vec![
                        SettingItem::new(
                            t!("settings.general.appearance.dark_mode.label").to_string(),
                            SettingField::switch(
                                |cx: &App| cx.theme().mode.is_dark(),
                                |val: bool, cx: &mut App| {
                                    let mode = if val {
                                        ThemeMode::Dark
                                    } else {
                                        ThemeMode::Light
                                    };
                                    Theme::global_mut(cx).mode = mode;
                                    Theme::change(mode, None, cx);
                                },
                            )
                            .default_value(false),
                        )
                        .description(
                            t!("settings.general.appearance.dark_mode.description").to_string(),
                        ),
                        SettingItem::new(
                            t!("settings.general.appearance.auto_switch.label").to_string(),
                            SettingField::checkbox(
                                |cx: &App| AppSettings::global(cx).auto_switch_theme,
                                |val: bool, cx: &mut App| {
                                    AppSettings::global_mut(cx).auto_switch_theme = val;
                                },
                            )
                            .default_value(default_settings.auto_switch_theme),
                        )
                        .description(
                            t!("settings.general.appearance.auto_switch.description").to_string(),
                        ),
                        SettingItem::new(
                            t!("settings.general.appearance.resettable.label").to_string(),
                            SettingField::switch(
                                |cx: &App| AppSettings::global(cx).resettable,
                                |checked: bool, cx: &mut App| {
                                    AppSettings::global_mut(cx).resettable = checked
                                },
                            ),
                        )
                        .description(
                            t!("settings.general.appearance.resettable.description").to_string(),
                        ),
                        SettingItem::new(
                            t!("settings.general.appearance.group_variant.label").to_string(),
                            SettingField::dropdown(
                                vec![
                                    (
                                        GroupBoxVariant::Normal.as_str().into(),
                                        t!("settings.general.appearance.group_variant.normal")
                                            .to_string()
                                            .into(),
                                    ),
                                    (
                                        GroupBoxVariant::Outline.as_str().into(),
                                        t!("settings.general.appearance.group_variant.outline")
                                            .to_string()
                                            .into(),
                                    ),
                                    (
                                        GroupBoxVariant::Fill.as_str().into(),
                                        t!("settings.general.appearance.group_variant.fill")
                                            .to_string()
                                            .into(),
                                    ),
                                ],
                                |cx: &App| AppSettings::global(cx).group_variant.clone(),
                                |val: SharedString, cx: &mut App| {
                                    AppSettings::global_mut(cx).group_variant = val;
                                },
                            )
                            .default_value(default_settings.group_variant),
                        )
                        .description(
                            t!("settings.general.appearance.group_variant.description").to_string(),
                        ),
                        SettingItem::new(
                            t!("settings.general.appearance.group_size.label").to_string(),
                            SettingField::dropdown(
                                vec![
                                    (
                                        Size::Medium.as_str().into(),
                                        t!("settings.general.appearance.group_size.medium")
                                            .to_string()
                                            .into(),
                                    ),
                                    (
                                        Size::Small.as_str().into(),
                                        t!("settings.general.appearance.group_size.small")
                                            .to_string()
                                            .into(),
                                    ),
                                    (
                                        Size::XSmall.as_str().into(),
                                        t!("settings.general.appearance.group_size.xsmall")
                                            .to_string()
                                            .into(),
                                    ),
                                ],
                                |cx: &App| AppSettings::global(cx).size.clone(),
                                |val: SharedString, cx: &mut App| {
                                    AppSettings::global_mut(cx).size = val;
                                },
                            )
                            .default_value(default_settings.size),
                        )
                        .description(
                            t!("settings.general.appearance.group_size.description").to_string(),
                        ),
                    ]),
                SettingGroup::new()
                    .title(t!("settings.general.group.font").to_string())
                    .item(
                        SettingItem::new(
                            t!("settings.general.font.family.label").to_string(),
                            SettingField::dropdown(
                                vec![
                                    ("Arial".into(), "Arial".into()),
                                    ("Helvetica".into(), "Helvetica".into()),
                                    ("Times New Roman".into(), "Times New Roman".into()),
                                    ("Courier New".into(), "Courier New".into()),
                                ],
                                |cx: &App| AppSettings::global(cx).font_family.clone(),
                                |val: SharedString, cx: &mut App| {
                                    AppSettings::global_mut(cx).font_family = val;
                                },
                            )
                            .default_value(default_settings.font_family),
                        )
                        .description(t!("settings.general.font.family.description").to_string()),
                    )
                    .item(
                        SettingItem::new(
                            t!("settings.general.font.size.label").to_string(),
                            SettingField::number_input(
                                NumberFieldOptions {
                                    min: 8.0,
                                    max: 72.0,
                                    ..Default::default()
                                },
                                |cx: &App| AppSettings::global(cx).font_size,
                                |val: f64, cx: &mut App| {
                                    // Only update AppSettings - Theme will auto-sync
                                    AppSettings::global_mut(cx).font_size = val;
                                },
                            )
                            .default_value(default_settings.font_size),
                        )
                        .description(t!("settings.general.font.size.description").to_string()),
                    )
                    .item(
                        SettingItem::new(
                            t!("settings.general.font.line_height.label").to_string(),
                            SettingField::number_input(
                                NumberFieldOptions {
                                    min: 8.0,
                                    max: 32.0,
                                    ..Default::default()
                                },
                                |cx: &App| AppSettings::global(cx).line_height,
                                |val: f64, cx: &mut App| {
                                    AppSettings::global_mut(cx).line_height = val;
                                },
                            )
                            .default_value(default_settings.line_height),
                        )
                        .description(
                            t!("settings.general.font.line_height.description").to_string(),
                        ),
                    ),
                SettingGroup::new()
                    .title(t!("settings.general.group.other").to_string())
                    .items(vec![
                        SettingItem::render(|options, _, _| {
                            h_flex()
                                .w_full()
                                .justify_between()
                                .flex_wrap()
                                .gap_3()
                                .child(t!("settings.general.other.custom_item").to_string())
                                .child(
                                    Button::new("action")
                                        .icon(IconName::Globe)
                                        .label(
                                            t!("settings.general.other.repository.button")
                                                .to_string(),
                                        )
                                        .outline()
                                        .with_size(options.size)
                                        .on_click(|_, _, cx| {
                                            cx.open_url(
                                                "https://github.com/sxhxliang/agent_studio",
                                            );
                                        }),
                                )
                        }),
                        SettingItem::new(
                            t!("settings.general.other.cli_path.label").to_string(),
                            SettingField::input(
                                |cx: &App| AppSettings::global(cx).cli_path.clone(),
                                |val: SharedString, cx: &mut App| {
                                    println!("cli-path set value: {}", val);
                                    AppSettings::global_mut(cx).cli_path = val;
                                },
                            )
                            .default_value(default_settings.cli_path),
                        )
                        .layout(Axis::Vertical)
                        .description(t!("settings.general.other.cli_path.description").to_string()),
                        SettingItem::new(
                            t!("settings.general.other.nodejs_path.label").to_string(),
                            SettingField::input(
                                |cx: &App| AppSettings::global(cx).nodejs_path.clone(),
                                |val: SharedString, cx: &mut App| {
                                    log::info!("nodejs_path set to: {}", val);
                                    AppSettings::global_mut(cx).nodejs_path = val;
                                },
                            )
                            .default_value(default_settings.nodejs_path),
                        )
                        .layout(Axis::Vertical)
                        .description(
                            t!("settings.general.other.nodejs_path.description").to_string(),
                        ),
                    ]),
            ])
    }
}
