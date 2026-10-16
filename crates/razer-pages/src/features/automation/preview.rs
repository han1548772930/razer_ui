use super::*;

pub fn open_preview(window: &mut Window, cx: &mut App) {
    let page = cx.new(|_| Automation::empty(0, 0));
    page.update(cx, |page, cx| {
        page.preview = true;
        page.chroma_installed = Some(true);
        for kind in 1..=5 {
            page.catalogs.insert(
                kind,
                vec![json!({
                    "id": format!("sample-{kind}"),
                    "guid": format!("sample-{kind}"),
                    "name": format!("Sample {}", text(&spec().actions[kind as usize].content))
                })],
            );
        }
        cx.notify();
    });
    let preview = cx.new(|_| AutomationPreview { page });
    window.open_dialog(cx, move |dialog, window, _| {
        dialog
            .title("Base Station V3 Chroma - Automation Preview")
            .w(window.rem_size() * (1000. / 16.))
            .child(preview.clone())
    });
}

struct AutomationPreview {
    page: Entity<Automation>,
}

impl Render for AutomationPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .child(surface::note(
                "Isolated UI preview. Audio devices, games, macros and shortcuts are examples; Save only changes this preview and never executes automation.",
                cx,
            ))
            .child(h_flex().gap_2().children(
                [(false, "Catalogs populated"), (true, "Catalogs empty")].map(
                    |(empty, label)| {
                        Button::new(SharedString::from(format!(
                            "automation-preview-empty-{empty}"
                        )))
                        .outline()
                        .label(label)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.page.update(cx, |page, cx| {
                                page.catalogs.clear();
                                if !empty {
                                    for kind in 1..=5 {
                                        page.catalogs.insert(
                                            kind,
                                            vec![json!({
                                                "id": format!("sample-{kind}"),
                                                "guid": format!("sample-{kind}"),
                                                "name": format!(
                                                    "Sample {}",
                                                    text(&spec().actions[kind as usize].content)
                                                )
                                            })],
                                        );
                                    }
                                }
                                cx.notify();
                            });
                        }))
                    },
                ),
            ))
            .child(
                div()
                    .id("automation-preview-scroll")
                    .h(surface::css(480.))
                    .scrollable_both()
                    .child(self.page.clone()),
            )
    }
}
