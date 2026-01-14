use gpui::{App, Entity, SharedString};
use gpui_component::setting::{
    NumberFieldOptions, SettingField, SettingGroup, SettingItem, SettingPage,
};
use rust_i18n::t;

use super::panel::SettingsPanel;

impl SettingsPanel {
    pub fn network_page(&self, _view: &Entity<Self>) -> SettingPage {
        SettingPage::new(t!("settings.network.title").to_string())
            .default_open(false)
            .groups(vec![
                SettingGroup::new()
                    .title(t!("settings.network.group.proxy").to_string())
                    .items(vec![
                        SettingItem::new(
                            t!("settings.network.proxy.enable.label").to_string(),
                            SettingField::switch(
                                |_cx: &App| false,
                                |val: bool, _cx: &mut App| {
                                    log::info!("Proxy enabled: {}", val);
                                    // TODO: Implement proxy config update
                                },
                            )
                            .default_value(false),
                        )
                        .description(t!("settings.network.proxy.enable.description").to_string()),
                        SettingItem::new(
                            t!("settings.network.proxy.type.label").to_string(),
                            SettingField::dropdown(
                                vec![
                                    (
                                        SharedString::from("http"),
                                        SharedString::from(t!("settings.network.proxy.type.http")),
                                    ),
                                    (
                                        SharedString::from("https"),
                                        SharedString::from(t!("settings.network.proxy.type.https")),
                                    ),
                                    (
                                        SharedString::from("socks5"),
                                        SharedString::from(t!(
                                            "settings.network.proxy.type.socks5"
                                        )),
                                    ),
                                ],
                                |_cx: &App| SharedString::from("http"),
                                |val: SharedString, _cx: &mut App| {
                                    log::info!("Proxy type: {}", val);
                                    // TODO: Implement proxy type update
                                },
                            )
                            .default_value(SharedString::from("http")),
                        )
                        .description(t!("settings.network.proxy.type.description").to_string()),
                        SettingItem::new(
                            t!("settings.network.proxy.host.label").to_string(),
                            SettingField::input(
                                |_cx: &App| SharedString::from(""),
                                |val: SharedString, _cx: &mut App| {
                                    log::info!("Proxy host: {}", val);
                                    // TODO: Implement proxy host update
                                },
                            )
                            .default_value(SharedString::from("")),
                        )
                        .description(t!("settings.network.proxy.host.description").to_string()),
                        SettingItem::new(
                            t!("settings.network.proxy.port.label").to_string(),
                            SettingField::number_input(
                                NumberFieldOptions {
                                    min: 1.0,
                                    max: 65535.0,
                                    ..Default::default()
                                },
                                |_cx: &App| 8080.0,
                                |val: f64, _cx: &mut App| {
                                    log::info!("Proxy port: {}", val);
                                    // TODO: Implement proxy port update
                                },
                            )
                            .default_value(8080.0),
                        )
                        .description(t!("settings.network.proxy.port.description").to_string()),
                    ]),
                SettingGroup::new()
                    .title(t!("settings.network.group.auth").to_string())
                    .items(vec![
                        SettingItem::new(
                            t!("settings.network.auth.username.label").to_string(),
                            SettingField::input(
                                |_cx: &App| SharedString::from(""),
                                |val: SharedString, _cx: &mut App| {
                                    log::info!("Proxy username set");
                                    // TODO: Implement proxy username update
                                },
                            )
                            .default_value(SharedString::from("")),
                        )
                        .description(t!("settings.network.auth.username.description").to_string()),
                        SettingItem::new(
                            t!("settings.network.auth.password.label").to_string(),
                            SettingField::input(
                                |_cx: &App| SharedString::from(""),
                                |val: SharedString, _cx: &mut App| {
                                    log::info!("Proxy password set");
                                    // TODO: Implement proxy password update
                                },
                            )
                            .default_value(SharedString::from("")),
                        )
                        .description(t!("settings.network.auth.password.description").to_string()),
                    ]),
            ])
    }
}
