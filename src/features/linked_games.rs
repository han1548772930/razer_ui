//! The source launches /synapse/profiles/index.html?displayMode=embeded.
//! This local list records executables only; it never starts or monitors games.
use super::*;
use std::path::Path;

pub(super) fn executable_key(path: &str) -> String {
    path.replace('/', "\\").to_lowercase()
}

pub(super) fn valid_game(game: &LinkedGame) -> bool {
    let path = game.executable.as_str();
    let bytes = path.as_bytes();
    let absolute = Path::new(path).is_absolute()
        || bytes.len() > 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'\\' | b'/')
        || path.starts_with("\\\\");
    let file = path.rsplit(['/', '\\']).next().unwrap_or_default();
    !game.name.trim().is_empty()
        && game.name.encode_utf16().count() <= 128
        && !game.name.chars().any(char::is_control)
        && absolute
        && path.len() <= 4096
        && !path.chars().any(char::is_control)
        && file.len() > 4
        && file.to_ascii_lowercase().ends_with(".exe")
}

fn game_from_path(path: &Path) -> Result<LinkedGame, String> {
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("无法读取程序文件：{error}"))?;
    if !canonical.is_file() {
        return Err("请选择一个 .exe 程序文件。".into());
    }
    let game = LinkedGame {
        name: path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .into(),
        executable: canonical.to_string_lossy().into_owned(),
    };
    valid_game(&game)
        .then_some(game)
        .ok_or_else(|| "请选择有效的 .exe 程序文件。".into())
}

struct LinkedGamesDialog {
    target: ProfileTarget,
    original: Vec<LinkedGame>,
    games: Vec<LinkedGame>,
    busy: bool,
    error: Option<String>,
}

impl LinkedGamesDialog {
    fn add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.error = None;
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("选择要关联的 .exe 文件".into()),
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = match picker.await {
                Ok(Ok(Some(paths))) => {
                    let path = paths.into_iter().next();
                    match path {
                        Some(path) => Some(
                            cx.background_executor()
                                .spawn(async move { game_from_path(&path) })
                                .await,
                        ),
                        None => None,
                    }
                }
                Ok(Ok(None)) => None,
                _ => Some(Err("无法打开文件选择窗口。".into())),
            };
            _ = view.update_in(cx, |this, _, cx| {
                this.busy = false;
                if this.target.current(cx).is_none() {
                    this.error = Some("当前配置文件已切换，请关闭后重新打开关联游戏。".into());
                } else if let Some(result) = result {
                    match result {
                        Ok(game)
                            if this.games.iter().any(|old| {
                                executable_key(&old.executable) == executable_key(&game.executable)
                            }) =>
                        {
                            this.error = Some("此程序已经关联到该配置文件。".into());
                        }
                        Ok(_) if this.games.len() >= 128 => {
                            this.error = Some("最多关联 128 个程序。".into())
                        }
                        Ok(game) => this.games.push(game),
                        Err(error) => this.error = Some(error),
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(workspace) = self.target.current(cx) else {
            self.error = Some("当前配置文件已切换，请关闭后重新打开关联游戏。".into());
            cx.notify();
            return;
        };
        if self.busy {
            return;
        }
        // Do not overwrite an edit made while the dialog was open.
        if workspace.read(cx).settings().linked_games != self.original {
            self.error = Some("游戏关联已发生变化，请关闭后重新打开。".into());
            cx.notify();
            return;
        }
        let games = self.games.clone();
        workspace.update(cx, |workspace, cx| {
            workspace.edit(window, cx, |settings| settings.linked_games = games);
        });
        dismiss_profile_dialog(&self.target.workspace, window, cx);
    }
}

impl Render for LinkedGamesDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut body = v_flex()
            .id("linked-games-content")
            .w_full()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p(surface::css(20.))
            .gap(surface::css(10.))
            .child(surface::note(
                "在本地配置中记录关联程序。保存后不会自动启动游戏或切换配置文件。",
                cx,
            ));
        if self.games.is_empty() {
            body = body.child(
                div()
                    .id("linked-games-empty")
                    .test_support()
                    .child("尚未关联游戏。"),
            );
        } else {
            body =
                body.child(
                    v_flex()
                        .id("linked-games-list")
                        .max_h(surface::css(300.))
                        .overflow_y_scroll()
                        .children(self.games.iter().map(|game| {
                            let path = game.executable.clone();
                            h_flex()
                                .gap_3()
                                .items_center()
                                .py_2()
                                .border_b_1()
                                .border_color(cx.theme().border)
                                .child(
                                    v_flex().flex_1().min_w_0().child(game.name.clone()).child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(path.clone()),
                                    ),
                                )
                                .child(
                                    Button::new(SharedString::from(format!(
                                        "linked-game-remove-{}",
                                        path
                                    )))
                                    .label("移除")
                                    .ghost()
                                    .disabled(self.busy)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.games.retain(|game| game.executable != path);
                                        this.error = None;
                                        cx.notify();
                                    })),
                                )
                        })),
                );
        }
        let body = body
            .child(
                profile_dialog_button("linked-games-add", "添加程序…", cx)
                    .disabled(self.busy)
                    .on_click(cx.listener(|this, _, window, cx| this.add(window, cx))),
            )
            .when_some(self.error.clone(), |body, error| {
                body.child(
                    div()
                        .id("linked-games-error")
                        .test_support()
                        .text_color(cx.theme().danger)
                        .child(error),
                )
            });
        v_flex()
            .id("linked-games-dialog")
            .test_support()
            .size_full()
            .min_h_0()
            .child(body)
            .child(
                profile_dialog_footer(cx)
                    .child(
                        profile_dialog_button("linked-games-cancel", "取消", cx).on_click(
                            cx.listener(|this, _, window, cx| {
                                dismiss_profile_dialog(&this.target.workspace, window, cx)
                            }),
                        ),
                    )
                    .child(
                        profile_dialog_button("linked-games-save", "保存到本地草稿", cx)
                            .primary()
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
                    ),
            )
    }
}

impl DeviceWorkspace {
    pub(super) fn open_linked_games(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(profile) = self.device.active_profile_obj() else {
            return;
        };
        let name = profile.name.clone();
        let games = self.settings().linked_games.clone();
        let target = ProfileTarget::new(self, cx);
        let view = cx.new(|_| LinkedGamesDialog {
            target,
            original: games.clone(),
            games,
            busy: false,
            error: None,
        });
        self.show_profile_dialog(format!("关联游戏 — {name}"), view.into(), window, cx);
    }
}

#[cfg(test)]
#[path = "linked_games_tests.rs"]
mod tests;
