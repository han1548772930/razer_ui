//! 已关联游戏磁贴（`.linked-game-tile`）与「添加」磁贴。
//!
//! # 依据
//!
//! - 结构：`.ref/applications/synapse/profiles/static/js/1255.b556ae48.chunk.js`
//!   的 `renderGameTiles()`：
//!   ```jsx
//!   <div className="list-box">
//!     {games.map(game => <GameTile …/>)}
//!     <div className="linked-game-tile add-new" onClick={openAddAppPopup}>
//!       <div className="game-body"><div className="plus-icon"/></div>
//!       <div className="game-footer"><label>…</label></div>
//!     </div>
//!   </div>
//!   ```
//! - 样式：`.ref/applications/synapse/profiles/static/css/1255.d3d31cbf.chunk.css`
//!   与 `9449.b8e7f39a.chunk.css`：
//!   `.list-box{display:flex;flex-wrap:wrap;margin-right:-5px}`、
//!   `.linked-game-tile{background-color:#111;border:1px solid #111;border-radius:5px;
//!    margin:10px 5px 0;height:190px;width:240px;position:relative}`、
//!   `:hover{border:1px solid #44d62c}`、`:active{border:2px solid #44d62c}`、
//!   `.active{border:2px solid #44d62c!important}`、
//!   `.add-new{background:#0000;border:2px dashed #5d5d5d;transition:border-color .2s}`、
//!   `.add-new:hover,:active{border-color:#44d62c}`、
//!   `.game-body{…;border-radius:5px 5px 0 0;display:flex;height:120px;justify-content:center}`、
//!   `.game-footer{height:70px;padding:10px;width:100%}`、
//!   `.game-footer .name{color:#ccc;font-size:14px;line-height:19px;margin-bottom:3px;
//!    text-align:center;text-overflow:ellipsis;text-transform:…}`、
//!   `.plus-icon{left:50%;margin:auto;position:absolute;top:50%;transform:translate(-50%,-50%)}`。
use crate::{i18n, ui::surface};
use gpui_kit::base::Button as BaseButton;
use gpui_kit::component::*;
use gpui_kit::*;

/// 磁贴尺寸（`.linked-game-tile,.linked-game{height:190px;width:240px}`）。
pub(crate) const TILE_WIDTH: f32 = 240.;
pub(crate) const TILE_HEIGHT: f32 = 190.;
/// `.game-body{height:120px}` + `.game-footer{height:70px}` = 190px。
pub(crate) const BODY_HEIGHT: f32 = 120.;
pub(crate) const FOOTER_HEIGHT: f32 = 70.;
/// `.plus-icon{height:40px;width:40px;background-size:40px}`。
pub(crate) const PLUS_ICON: f32 = 40.;

/// `.list-box`：磁贴容器。
pub(crate) fn list_box(children: impl IntoIterator<Item = AnyElement>) -> AnyElement {
    h_flex()
        .flex_wrap()
        .mr(-surface::css(5.))
        .children(children)
        .into_any_element()
}

// 每个游戏一张磁贴的 `linked_game_tile(…)` 需要真实的已关联游戏数据（游戏封面、
// 关联的配置文件、未识别状态）。本地没有该数据源，且它在本仓库里会变成死代码，
// 因此这里只保留源码里**空列表也会渲染**的那张 `.add-new` 磁贴；数据源接入后
// 再按同一份 CSS 补回逐游戏磁贴（规则已记录在
// `docs/re/linked-game-tile-audit.md`）。

/// 一张已关联游戏磁贴：`.linked-game-tile`。
///
/// 源码里 `.game-body` 放游戏封面（`.game-body{background-size:cover}`），没有封面时
/// 用 `.game-body.ico-image{background-size:60px 60px}` 放程序图标。本地没有游戏封面
/// 与程序图标数据源，因此这里保留 120px 的游戏区但**不画任何图**（不伪造封面），
/// 页脚按源码渲染 `.game-footer .name`，并保留本对话框原有的「移除」入口。
pub(crate) fn linked_game_tile(
    id: SharedString,
    name: String,
    on_remove: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> AnyElement {
    v_flex()
        .id(id)
        .relative()
        .w(surface::css(TILE_WIDTH))
        .h(surface::css(TILE_HEIGHT))
        .flex_shrink_0()
        .ml(surface::css(5.))
        .mr(surface::css(5.))
        .mt(surface::css(10.))
        .bg(cx.theme().group_box)
        .border_1()
        .border_color(cx.theme().group_box)
        .rounded(surface::css(5.))
        .overflow_hidden()
        .hover(|s| s.border_color(cx.theme().primary))
        .active(|s| s.border_2().border_color(cx.theme().primary))
        .child(div().h(surface::css(BODY_HEIGHT)).w_full())
        .child(
            v_flex()
                .h(surface::css(FOOTER_HEIGHT))
                .w_full()
                .p(surface::css(10.))
                .gap(surface::css(4.))
                .child(
                    div()
                        .text_size(surface::css(14.))
                        .line_height(surface::css(19.))
                        .truncate()
                        .text_center()
                        .text_color(cx.theme().foreground)
                        .child(name),
                )
                .child(
                    BaseButton::new("linked-game-remove")
                        .p_0()
                        .text_size(surface::css(12.))
                        .line_height(surface::css(14.))
                        .text_color(cx.theme().muted_foreground)
                        .underline()
                        .hover(|button| button.text_color(cx.theme().primary))
                        .child(i18n::t_or("DELETE", "删除"))
                        .on_click(on_remove),
                ),
        )
        .into_any_element()
}

/// 「添加游戏/程序」磁贴：`.linked-game-tile.add-new`。
/// `label` 取源码文案 key（见调用方），点按由调用方处理。
pub(crate) fn add_new_tile(
    id: &'static str,
    label: String,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> AnyElement {
    v_flex()
        .id(id)
        .relative()
        .w(surface::css(TILE_WIDTH))
        .h(surface::css(TILE_HEIGHT))
        .flex_shrink_0()
        .ml(surface::css(5.))
        .mr(surface::css(5.))
        .mt(surface::css(10.))
        // `.add-new{background:#0000;border:2px dashed #5d5d5d;transition:border-color .2s}`
        .border_2()
        .border_dashed()
        .border_color(cx.theme().border)
        .rounded(surface::css(5.))
        .hover(|s| s.border_color(cx.theme().primary))
        .active(|s| s.border_color(cx.theme().primary))
        .on_click(on_click)
        .child(
            div()
                .relative()
                .h(surface::css(BODY_HEIGHT))
                .w_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    // `.plus-icon{background-image:url(icon_add.c95a8d74.svg);
                    //  background-size:40px;height:40px;width:40px;margin:50px auto}` +
                    // `.game-tile.add-new .plus-icon{left:50%;top:50%;
                    //  transform:translate(-50%,-50%);position:absolute;margin:auto}`
                    // 资产就是 182 media 里的 `icon_add.c95a8d74.svg`（本地打包名
                    // `synapse/dashboard-add.svg`）。
                    img("synapse/dashboard-add.svg")
                        .size(surface::css(PLUS_ICON))
                        .flex_shrink_0(),
                ),
        )
        .child(
            div()
                .h(surface::css(FOOTER_HEIGHT))
                .w_full()
                .flex()
                .items_center()
                .justify_center()
                .text_size(surface::css(14.))
                .line_height(surface::css(16.))
                .text_center()
                .text_color(cx.theme().muted_foreground)
                .child(label),
        )
        .into_any_element()
}

/// 该磁贴组当前使用的「添加」文案 key（源码里是三个片段拼在一行 label 里，
/// 符号在压缩后被改名，无法逐一还原，因此这里用语义最接近的一条已存在 key）。
pub(crate) fn add_new_label() -> String {
    i18n::t_or("ADD_GAME", "添加游戏")
}
