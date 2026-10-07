//! 1383 Razer Kraken V4 Pro 的 OLED 根：当前 `6141.5d00192e.chunk.js` 的 `xx`
//! （约 661194–662008）与它挂载的五个 `.widget`（`Nv` 亮度、`Bv` 语言、`Ov` 回主屏时间、
//! `Uv` 息屏变暗、`Kv` 屏保），几何与颜色取自 `6141.d8b30400.chunk.css` 与
//! `main.7a0131e4.css`。
//!
//! 原版 `xx` 的两列归属是 `widget-col col-left`（亮度、语言）与
//! `widget-col col-right`（回主屏时间、息屏变暗、屏保），本模块按该归属分发描述符里的分区。
//! `Dv`（Home Screen Display 卡片网格）及 `Kg` Media 编辑器由 `audio_oled_home`
//! 依据 1383 自身的 reducer 初始预览数据实现；真实硬件与其余编辑器边界见审计文档。
use super::*;
use crate::ui::source_slider::SourceSlider;

/// 原版 `xx` 的 `widget-col col-left`：亮度与语言同列，其余进右列。
const KRAKEN_OLED_LEFT_COLUMN: [&str; 2] = ["OLED_BRIGHTNESS_TITLE", "OLED_LANGUAGE_TITLE"];

impl AudioProductWorkspace {
    /// 原版 `xx`：`body-widgets` 里两列 `.widget-col`，每列一个竖排 `.widget` 堆叠。
    pub(super) fn kraken_oled_page(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(page) = self.spec.pages.iter().find(|page| page.key == "TAB_OLED") else {
            return div().into_any_element();
        };
        let mut left = Vec::new();
        let mut right = Vec::new();
        for section in &page.sections {
            let panel = self.kraken_oled_section(section, cx);
            if KRAKEN_OLED_LEFT_COLUMN.contains(&section.title.as_str()) {
                left.push(panel);
            } else {
                right.push(panel);
            }
        }
        v_flex()
            .id("kraken-oled-page")
            .test_support()
            .relative()
            .max_w(surface::css(1240.))
            .mx_auto()
            .child(self.kraken_oled_home(window, cx))
            .child(
                surface::page_columns()
                    .child(surface::page_column(v_flex().children(left)))
                    .child(surface::page_column(v_flex().children(right))),
            )
            .children(self.oled_runtime_layers(window, cx))
            .into_any_element()
    }

    fn kraken_oled_section(
        &self,
        section: &'static AudioSection,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match section.title.as_str() {
            "OLED_BRIGHTNESS_TITLE" => self.kraken_oled_brightness(section, cx),
            "OLED_LANGUAGE_TITLE" => self.kraken_oled_language(section, cx),
            // `Ov` 的 `.polling-btn-set` 是 `marginTop:22px`。
            "OLED_TIME_TO_HOME_SCREEN_TITLE" => self.kraken_oled_options(section, 22., false, cx),
            // `Uv` 的 `.polling-btn-set` 是 `marginTop:20px`，并带禁用遮罩。
            "OLED_DIM_DISPLAY_TITLE" => self.kraken_oled_options(section, 20., true, cx),
            "OLED_SCREEN_SAVER_TITLE" => self.kraken_oled_screensaver(section, cx),
            _ => self.kraken_oled_generic(section, cx),
        }
    }

    /// `Nv`：说明 + 一个 `<br/>` + `uo.A` 亮度滑条（`min:30,minTag:30,max:100`）。
    fn kraken_oled_brightness(
        &self,
        section: &'static AudioSection,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let control = section.controls.first().expect("1383 OLED brightness row");
        let min = control.min;
        let max = control.max;
        let value = self
            .draft
            .pointer(&control.path)
            .and_then(Value::as_f64)
            .unwrap_or(f64::from(min)) as f32;
        let progress = if (max - min).abs() < f32::EPSILON {
            0.
        } else {
            ((value - min) / (max - min)).clamp(0., 1.)
        };
        let mut panel = kraken_oled_panel(section, cx);
        if let Some(description) = &section.description {
            panel = panel.child(div().child(t(description)));
        }
        // 原版说明与滑条之间的 `<br/>`：一行 14px Roboto 行盒。
        panel = panel.child(div().h(surface::css(17.)));
        panel
            .child(kraken_oled_brightness_slider(
                &self.sliders[&control.path],
                value,
                progress,
                min,
                max,
            ))
            .into_any_element()
    }

    /// `Bv`：下拉框只改本地暂存值，`APPLY` 才写回（`.OLEDLanguage_button`/`_disabled`）。
    fn kraken_oled_language(
        &self,
        section: &'static AudioSection,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let control = section.controls.first().expect("1383 OLED language row");
        let label = control
            .apply_label
            .as_deref()
            .expect("1383 OLED language apply label");
        let (raw, staged, selected) = self.oled_language_values();
        let unchanged = staged == raw && staged == selected;
        let disabled = self.oled_is_ble() || self.oled_is_loading();
        let apply_button = gpui_kit::base::Button::new("kraken-oled-language-apply")
            .accessibility_label(t(label))
            .disabled(unchanged)
            .styles(|style| style.disabled(|style| style.opacity(0.3)))
            .h(surface::css(27.))
            .px(surface::css(15.))
            .rounded(surface::css(3.))
            .bg(rgb(0x44d62c))
            .text_color(rgb(0))
            .text_size(surface::css(14.))
            .child(t(label).to_uppercase());
        let apply_button = apply_button.on_click(cx.listener(move |this, _, window, cx| {
            this.apply_oled_language(window, cx);
        }));
        let mut panel = kraken_oled_panel(section, cx)
            .child(div().child(t(&control.label)))
            // `.OLEDLanguage_dropdown{margin:10px 0 20px;display:flex}`。
            .child(
                h_flex()
                    .my(surface::css(10.))
                    .mb(surface::css(20.))
                    .gap(surface::css(10.))
                    .child(surface::select(&self.selects[&control.path]))
                    .child(apply_button),
            );
        if let Some(description) = &section.description {
            // `.OLEDLanguage_desc{color:#999}`。
            panel = panel.child(div().text_color(rgb(0x999999)).child(t(description)));
        }
        div()
            .relative()
            .child(panel)
            .when(disabled, |el| {
                el.opacity(0.3).child(div().absolute().inset_0().occlude())
            })
            .into_any_element()
    }

    /// `Ov`/`Uv`：`.polling-btn-set` 里每个 `.customize-polling-rate-button.keyboard-btn-size`
    /// 都是 48×27（`DimKeyboardLighting_width-auto__C0C3F` 覆盖 65px 宽）。
    fn kraken_oled_options(
        &self,
        section: &'static AudioSection,
        top: f32,
        backdrop: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let control = section.controls.first().expect("1383 OLED option row");
        let disabled = !self.enabled(control);
        let selected = self.draft.pointer(&control.path).cloned();
        let path = control.path.clone();
        let buttons: Vec<AnyElement> = control
            .options
            .iter()
            .map(|option| {
                let value = option.value.clone();
                let active = selected.as_ref() == Some(&value);
                let id = SharedString::from(format!(
                    "kraken-oled-{}-{}",
                    section.title.to_lowercase(),
                    value
                ));
                let click = value.clone();
                let click_path = path.clone();
                gpui_kit::base::Button::new(id)
                    .accessibility_label(t(&option.label))
                    // 源里这一行没有 disabled 类：禁用只由上层底衬表达，按钮本身不额外变暗。
                    .w(surface::css(48.))
                    .h(surface::css(27.))
                    .rounded(surface::css(3.))
                    .border_1()
                    .border_color(if active { rgb(0x44d62c) } else { rgb(0x5d5d5d) })
                    .bg(rgb(0x222222))
                    .text_color(rgb(0xcccccc))
                    .text_size(surface::css(14.))
                    .hover(|button| button.border_color(rgb(0x44d62c)))
                    .child(t(&option.label))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.edit(&click_path, click.clone(), window, cx)
                    }))
                    .into_any_element()
            })
            .collect();
        let mut panel = kraken_oled_panel(section, cx);
        if let Some(description) = &section.description {
            panel = panel.child(div().child(t(description)));
        }
        panel
            .child(
                h_flex()
                    .relative()
                    .flex_wrap()
                    .gap(surface::css(10.))
                    .mt(surface::css(top))
                    .children(buttons)
                    // `DimKeyboardLighting_backdrop`：`#111`、300×30、`opacity:.5`，盖住不可用的按钮行。
                    .when(backdrop && disabled, |row| {
                        row.child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .w(surface::css(300.))
                                .h(surface::css(30.))
                                .bg(rgb(0x111111))
                                .opacity(0.5)
                                .occlude(),
                        )
                    }),
            )
            .into_any_element()
    }

    /// `Kv`：`.OLEDScreensaver_oled-screensaver-options` 两列网格，选项 260×68。
    fn kraken_oled_screensaver(
        &self,
        section: &'static AudioSection,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let control = self
            .control("/device/oledScreensaver")
            .expect("1383 OLED screensaver row");
        let selected = self.draft.pointer(&control.path).cloned();
        let path = control.path.clone();
        let options: Vec<AnyElement> = control
            .options
            .iter()
            .map(|option| {
                let value = option.value.clone();
                let active = selected.as_ref() == Some(&value);
                let click = value.clone();
                let click_path = path.clone();
                gpui_kit::base::Button::new(SharedString::from(format!(
                    "kraken-oled-screensaver-{value}"
                )))
                .accessibility_label(t(&option.label))
                // 选中项是 `pointer-events:none`：仍保持 2px 绿框，只是不再接收指针事件。
                .occlude()
                .w(surface::css(260.))
                .h(surface::css(68.))
                .flex_shrink_0()
                .border_1()
                .bg(rgb(0))
                .text_color(rgb(0x999999))
                .text_size(surface::css(14.))
                .border_color(rgb(0x5d5d5d))
                .when(active, |button| {
                    button.border_2().border_color(rgb(0x44d62c))
                })
                .hover(|button| button.border_2().border_color(rgb(0x166809)))
                .child(match &option.image {
                    Some(source) => img(SharedString::from(source.clone()))
                        .w(surface::css(256.))
                        .h(surface::css(64.))
                        .into_any_element(),
                    None => div()
                        .child(format!("({})", t(&option.label)))
                        .into_any_element(),
                })
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.edit(&click_path, click.clone(), window, cx)
                }))
                .into_any_element()
            })
            .collect();
        let mut panel = kraken_oled_panel(section, cx);
        if let Some(description) = &section.description {
            panel = panel.child(div().child(t(description)));
        }
        panel
            .child(
                h_flex()
                    .w(surface::css(530.))
                    .mt(surface::css(20.))
                    .flex_wrap()
                    .gap(surface::css(10.))
                    .children(options),
            )
            .into_any_element()
    }

    /// 未单独取证的分区仍走通用音频控件渲染，不隐藏也不伪造。
    fn kraken_oled_generic(
        &self,
        section: &'static AudioSection,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut panel = kraken_oled_panel(section, cx);
        if let Some(description) = &section.description {
            panel = panel.child(div().child(t(description)));
        }
        panel
            .children(section.controls.iter().map(|c| self.render_control(c, cx)))
            .into_any_element()
    }
}

/// `.widget` + 右上角 `.help`（`position:absolute;right:10px;top:10px`）。
fn kraken_oled_panel(section: &AudioSection, cx: &App) -> Div {
    let panel = surface::panel(t(&section.title), cx).relative();
    match &section.tips {
        Some(tips) => panel.child(kraken_oled_help(&section.title, tips)),
        None => panel,
    }
}

pub(super) fn kraken_oled_help(id: &str, text: &str) -> AnyElement {
    div()
        .absolute()
        .top(surface::css(10.))
        .right(surface::css(10.))
        .child(surface::help_control(
            SharedString::from(format!("kraken-oled-help-{id}")),
            t(text),
        ))
        .into_any_element()
}

/// `uo.A` 的共享滑条标记（`.slider-container.brightness.on`）：绘制交给
/// `SourceSlider`，这里只保留源容器下沿 `-2px` 的 `.foot` 两端标签。
fn kraken_oled_brightness_slider(
    state: &Entity<SliderState>,
    value: f32,
    progress: f32,
    min: f32,
    max: f32,
) -> AnyElement {
    div()
        .relative()
        .w_full()
        .child(SourceSlider::new(state, progress).tip(Some(format!("{value:.0}"))))
        .child(
            div()
                .absolute()
                .bottom(surface::css(-2.))
                .w_full()
                .flex()
                .justify_between()
                .text_size(surface::css(14.))
                .text_color(rgb(0x999999))
                .child(format!("{min:.0}"))
                .child(format!("{max:.0}")),
        )
        .into_any_element()
}
