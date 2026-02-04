use gpui::{ParentElement as _, Styled};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable,
    label::Label,
    setting::{SettingField, SettingGroup, SettingItem, SettingPage},
    text::TextView,
    v_flex,
};
use rust_i18n::t;

use super::types::OpenURLSettingField;

pub fn about_page(resettable: bool) -> SettingPage {
    SettingPage::new(t!("settings.about.title").to_string())
        .resettable(resettable)
        .group(
            SettingGroup::new().item(SettingItem::render(|_options, _, cx| {
                v_flex()
                    .gap_3()
                    .w_full()
                    .items_center()
                    .justify_center()
                    .child(Icon::new(IconName::GalleryVerticalEnd).size_16())
                    .child(t!("settings.about.app_name").to_string())
                    .child(
                        Label::new(t!("settings.about.description").to_string())
                            .text_sm()
                            .text_color(cx.theme().muted_foreground),
                    )
            })),
        )
        .group(
            SettingGroup::new()
                .title(t!("settings.about.links.title").to_string())
                .items(vec![
                    SettingItem::new(
                        t!("settings.about.links.github.label").to_string(),
                        SettingField::element(OpenURLSettingField::new(
                            t!("settings.about.links.github.button").to_string(),
                            "https://github.com/sxhxliang/agent-studio",
                        )),
                    )
                    .description(t!("settings.about.links.github.description").to_string()),
                    SettingItem::new(
                        t!("settings.about.links.docs.label").to_string(),
                        SettingField::element(OpenURLSettingField::new(
                            t!("settings.about.links.docs.button").to_string(),
                            "https://docs.rs/gpui-component",
                        )),
                    )
                    .description(TextView::markdown(
                        "desc",
                        t!("settings.about.links.docs.description").to_string(),
                    )),
                    SettingItem::new(
                        t!("settings.about.links.website.label").to_string(),
                        SettingField::render(|options, _window, _cx| {
                            gpui_component::button::Button::new("open-url")
                                .outline()
                                .label(t!("settings.about.links.website.button").to_string())
                                .with_size(options.size)
                                .on_click(|_, _window, cx| {
                                    cx.open_url("https://github.com/sxhxliang/agent-studio");
                                })
                        }),
                    )
                    .description(t!("settings.about.links.website.description").to_string()),
                ]),
        )
}
