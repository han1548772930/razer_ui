//! 777 均衡器页：音频输出均衡器和可编辑频段。

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::shell::AppShell;
use crate::ui::widgets::{
    card, card_title, not_wired_hint, slider_row, toggle_button, EmptyState, PageLayout,
    SettingRow,
};

const EQ_BAND_IDS: [&str; 10] = [
    "eq-band-01",
    "eq-band-02",
    "eq-band-03",
    "eq-band-04",
    "eq-band-05",
    "eq-band-06",
    "eq-band-07",
    "eq-band-08",
    "eq-band-09",
    "eq-band-10",
];

pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };
    let Some(sound) = device.features.sound.as_ref() else {
        return PageLayout::new("均衡器", device.display_name())
            .subtitle("当前设备未声明音频均衡器能力")
            .widget(EmptyState::new("该设备没有均衡器设置"))
            .into_any_element();
    };

    let equalizer = &sound.equalizer;
    let bands = equalizer.bands.clone();
    let band_count = bands.len().min(EQ_BAND_IDS.len());

    let mut layout = PageLayout::new("均衡器", device.display_name())
        .subtitle("音频输出均衡器；频段调整保留在当前 profile")
        .widget(
            card()
                .child(card_title("音频均衡器"))
                .child(toggle_button(
                    "eq-enabled",
                    "启用均衡器",
                    equalizer.enabled,
                    cx,
                    |this, cx| this.toggle_eq(cx),
                ))
                .child(toggle_button(
                    "eq-esports",
                    "电竞均衡器",
                    equalizer.esports,
                    cx,
                    |this, cx| this.toggle_eq_esports(cx),
                ))
                .child(SettingRow::new("当前预设", equalizer.preset.clone()))
                .child(div().text_xs().child(
                    "所有电竞均衡器调整保存在耳机上；标准均衡器只保存自定义 profile。",
                )),
        )
        ;

    if band_count > 0 {
        layout = layout.widget(
            card()
                .child(card_title(format!("频段（{} 段，单位 dB）", band_count)))
                .children(bands.into_iter().take(band_count).enumerate().map(|(index, gain)| {
                    let id = EQ_BAND_IDS[index];
                    slider_row(
                        id,
                        format!("频段 {}", index + 1).as_str(),
                        gain as f32,
                        -12.,
                        12.,
                        1.,
                        format!("{gain:+} dB"),
                        equalizer.enabled,
                        cx,
                        move |this, value, cx| this.set_eq_band(index, value, cx),
                    )
                })),
        );
    }

    layout.widget(not_wired_hint()).into_any_element()
}
