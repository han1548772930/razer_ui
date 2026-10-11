//! Current PID190 Performance uS -> NS/dS and SS/AS. Local view state and
//! observed service state stay separate; emitted actions are never fake replies.
use gpui_kit::{component::{Disableable, StyledExt, h_flex, v_flex,
    input::{Input,InputEvent,InputState},select::{SelectEvent,SelectItem,SelectState}},
    base::Button as BaseButton,prelude::FluentBuilder as _,*};
use razer_i18n::t;
use razer_widgets::{surface,scroll::SourceScrollable as _};
use serde_json::Value;
use std::{cell::Cell,rc::Rc,time::Duration};
#[path="mouse_dpi_matcher_state.rs"]
mod state;
pub use state::{MatcherAction,MatcherCommand,MatcherObservation,MatcherState,ProfileItem};

impl SelectItem for ProfileItem {
    type Value=String;
    fn title(&self)->SharedString {self.name().to_owned().into()}
    fn value(&self)->&String {self.value_id()}
    fn disabled(&self)->bool {self.disabled()}
}

pub struct MouseDpiMatcher {
    state:MatcherState,
    selector:Entity<SelectState<Vec<ProfileItem>>>,
    rename_input:Entity<InputState>,
    syncing:bool,
    mounted:bool,
    body_scroll:ScrollHandle,
    calibrate_bounds:Rc<Cell<Bounds<Pixels>>>,
    _subscriptions:Vec<Subscription>,
}
impl EventEmitter<MatcherAction> for MouseDpiMatcher {}
impl MouseDpiMatcher {
    pub fn new(window:&mut Window,cx:&mut Context<Self>)->Self {
        let state=MatcherState::default();
        let (items,index)=state.dataset(&t("NONE"));
        let selector=cx.new(|cx|SelectState::new(items,Some(IndexPath::new(index)),window,cx));
        let rename_input=cx.new(|cx|InputState::new(window,cx).validate(|value,_|value.encode_utf16().count()<=32));
        let select_subscription=cx.subscribe_in(&selector,window,|this,_,event,window,cx|{
            if this.syncing {return;}
            if let SelectEvent::Confirm(Some(guid))=event {
                this.emit(this.state.choose(guid),cx);
                // Selection remains driven by the actual Redux-equivalent
                // observation from its parent, not a simulated service answer.
                this.sync_selector(window,cx);
            }
        });
        let rename_subscription=cx.subscribe_in(&rename_input,window,|this,input,event,window,cx|{
            if this.syncing||this.state.rename().is_none(){return;}
            match event {
                InputEvent::Blur => {let text=input.read(cx).value(cx).to_string();let actions=this.state.commit_rename(&text);this.emit(actions,cx);cx.notify();}
                InputEvent::PressEnter{..}=>window.blur(cx),
                _=>{}
            }
        });
        Self {state,selector,rename_input,syncing:false,mounted:false,body_scroll:ScrollHandle::default(),
            calibrate_bounds:Rc::new(Cell::new(Bounds::default())),_subscriptions:vec![select_subscription,rename_subscription]}
    }
    /// Invoke after the owner subscribes, only while the actual Performance
    /// component is mounted. Constructor/render do not emit GET_STATE.
    pub fn mount(&mut self,cx:&mut Context<Self>) {
        if self.mounted{return;}self.mounted=true;let actions=self.state.mount();self.emit(actions,cx);
    }
    pub fn unmount(&mut self,cx:&mut Context<Self>) {self.mounted=false;self.state.unmount();cx.notify();}
    pub fn observe(&mut self,observation:MatcherObservation,window:&mut Window,cx:&mut Context<Self>) {
        let actions=self.state.observe(observation);self.emit(actions,cx);self.sync_selector(window,cx);cx.notify();
    }
    pub fn state(&self)->&MatcherState {&self.state}
    /// Parent's browser-equivalent window click adapter must call this for
    /// clicks outside this entity. Stopped close/cancel/reset clicks are excluded.
    pub fn window_click(&mut self,position:Point<Pixels>,cx:&mut Context<Self>) {
        let inside=self.calibrate_bounds.get().contains(&position);let actions=self.state.window_click(inside);self.emit(actions,cx);cx.notify();
    }
    pub fn document_click(&mut self,inside_profile_actions:bool,cx:&mut Context<Self>) {
        self.state.document_click(inside_profile_actions);cx.notify();
    }
    fn emit(&self,actions:Vec<MatcherAction>,cx:&mut Context<Self>) {for action in actions{cx.emit(action);}}
    fn sync_selector(&mut self,window:&mut Window,cx:&mut Context<Self>) {
        let (items,index)=self.state.dataset(&t("NONE"));self.syncing=true;
        self.selector.update(cx,|select,cx|{select.set_items(items,window,cx);select.set_selected_index(Some(IndexPath::new(index)),window,cx);});self.syncing=false;
    }
    fn begin_rename(&mut self,window:&mut Window,cx:&mut Context<Self>) {
        if let Some(name)=self.state.begin_rename(&t("NONE")) {
            self.syncing=true;self.rename_input.update(cx,|input,cx|{input.set_value(name,window,cx);input.focus(window,cx);input.select_all(window,cx);});self.syncing=false;cx.notify();
        }
    }
    fn profile_bar(&self,window:&mut Window,cx:&mut Context<Self>)->AnyElement {
        let (items,index)=self.state.dataset(&t("NONE"));
        let sentinel=items.get(index).is_none_or(|item|item.guid()=="none");
        let disabled=items.len()<2&&sentinel;
        let selector=if self.state.rename().is_some() {
            div().ml(surface::css(15.)).mt(surface::css(1.)).w(surface::css(230.))
                .child(Input::new(&self.rename_input).h(surface::css(26.)))
                .on_key_down(cx.listener(|this,event:&KeyDownEvent,_,cx|{
                    if event.keystroke.key=="escape"&&this.state.rename().is_some() {
                        cx.stop_propagation();let actions=this.state.escape_rename();this.emit(actions,cx);cx.notify();
                    }
                })).into_any_element()
        }else{
            surface::select(&self.selector).items(items).placeholder("None").accessibility_label(t("SENSITIVITY_MATCHER"))
                .disabled(disabled).w(surface::css(230.)).into_any_element()
        };
        let mut dots=BaseButton::new("matcher190-profile-actions").accessibility_label(t("MORE"))
            .disabled(sentinel||self.state.rename().is_some()).relative().size(surface::css(26.)).mr(surface::css(10.))
            .border_1().border_color(if self.state.actions_open(){rgb(0x44d62c)}else{rgb(0x222222)})
            .rounded(surface::css(13.)).hover(|style|style.border_color(rgb(0x44d62c)))
            .child(img("synapse/mouse-matcher-190/more.svg").size(surface::css(20.)))
            .on_click(cx.listener(|this,_,_,cx|{this.state.toggle_actions();cx.notify();}));
        if self.state.actions_open() {
            let menu=v_flex().absolute().top(surface::css(26.)).left(surface::css(-1.)).min_w(surface::css(76.))
                .py(surface::css(2.5)).border_1().border_color(rgb(0x5d5d5d)).bg(rgb(0x000000))
                .child(menu_button("matcher190-rename","RENAME").on_click(cx.listener(|this,_,window,cx|{cx.stop_propagation();this.begin_rename(window,cx);})))
                .child(div().h(surface::css(1.)).mx(surface::css(6.)).my(surface::css(2.5)).bg(rgb(0x5d5d5d)))
                .child(menu_button("matcher190-delete","DELETE").on_click(cx.listener(|this,_,_,cx|{cx.stop_propagation();let actions=this.state.delete_profile();this.emit(actions,cx);cx.notify();})))
                .child(menu_button("matcher190-delete-all","DELETE_ALL").on_click(cx.listener(|this,_,_,cx|{cx.stop_propagation();this.state.open_delete_all();cx.notify();})))
                .on_mouse_up_out(MouseButton::Left,cx.listener(|this,_,_,cx|{this.state.document_click(false);cx.notify();}));
            dots=dots.child(deferred(menu).with_priority(111));
        }
        h_flex().w_full().h(surface::css(26.)).justify_start().text_size(surface::css(14.))
            .child(selector).child(dots).into_any_element()
    }
    fn delete_all_popup(&self,cx:&mut Context<Self>)->AnyElement {
        v_flex().absolute().top(surface::css(205.)).right(surface::css(70.)).w(surface::css(300.))
            .items_center().p(surface::css(20.)).gap(surface::css(10.)).bg(rgb(0x111111)).border_1().border_color(rgb(0xfd4949)).rounded(surface::css(3.))
            .child(div().text_color(rgb(0xfd4949)).font_weight(FontWeight::BOLD).text_center().child(t("DELETE_ALL_SENSITIVITY_MATCHER_POPUP_HEADER").to_uppercase()))
            .child(div().text_center().child(t("DELETE_ALL_SENSITIVITY_MATCHER_POPUP_DESC")))
            .child(h_flex().gap(surface::css(10.))
                .child(action_button("matcher190-delete-all-cancel","CANCEL",0x707070,0xffffff,90.).on_click(cx.listener(|this,_,_,cx|{this.state.close_delete_all();cx.notify();})))
                .child(action_button("matcher190-delete-all-confirm","DELETE",0xfd4949,0x111111,90.).on_click(cx.listener(|this,_,_,cx|{let actions=this.state.delete_all();this.emit(actions,cx);cx.notify();}))))
            .on_mouse_up_out(MouseButton::Left,cx.listener(|this,_,_,cx|{this.state.document_click(false);cx.notify();})).into_any_element()
    }
    fn step_content(&self,window:&mut Window,cx:&mut Context<Self>)->AnyElement {
        let phase=self.state.phase();
        if !matches!(phase,Some("ready"|"calibrating"|"error"|"completed")){return div().into_any_element();}
        let desc=match phase {Some("ready")=>"SENSITIVITY_MATCHER_ACTION_DESC_1",Some("completed")=>"SENSITIVITY_MATCHER_ACTION_DESC_3",_=>"SENSITIVITY_MATCHER_ACTION_DESC_2"};
        let mut content=v_flex().child(div().mt(surface::css(50.)).mb(surface::css(10.)).text_size(surface::css(14.)).line_height(surface::css(17.)).text_center().child(t(desc)));
        let mut artwork=h_flex().w(surface::css(450.)).h(surface::css(150.)).mt(surface::css(70.)).mx_auto();
        if phase==Some("completed") {
            artwork=artwork.justify_center().child(img("synapse/mouse-matcher-190/completed.svg").w(surface::css(136.)).h(surface::css(109.)));
        }else{
            artwork=artwork.child(img("synapse/mouse-matcher-190/handmouse.svg").w(surface::css(150.)).h(surface::css(136.)));
            if phase==Some("ready"){
                artwork=artwork.child(dotted(80.)).child(img("synapse/mouse-matcher-190/synapse.svg").w(surface::css(158.)).h(surface::css(103.)));
            }else{artwork=artwork.child(img("synapse/mouse-matcher-190/arrow.svg").w(surface::css(32.)).h(surface::css(26.)).ml(surface::css(50.)).mb(surface::css(50.)));}
        }
        content=content.child(artwork);
        if matches!(phase,Some("calibrating"|"error")) {
            let progress=gpui_kit::base::motion::transition(("matcher190-progress",cx.entity_id()),self.state.process(),gpui_kit::base::motion::Transition::new(Duration::from_millis(500)).easing(gpui_kit::base::motion::Easing::Ease),window,cx);
            content=content.child(div().mx_auto().mt(surface::css(10.)).w(surface::css(414.)).h(surface::css(7.)).bg(rgba(0x44d62c4d))
                .child(div().h_full().w(relative(progress/100.)).bg(rgb(0x44d62c))));
        }
        if phase==Some("completed") {
            content=content.child(div().mx_auto().font_family("RazerF5").text_size(surface::css(14.)).line_height(surface::css(14.)).h(surface::css(14.)).text_center().child(t("CALIBRATION_INFORMATION")))
                .child(v_flex().mx_auto().mt(surface::css(10.)).w(surface::css(118.)).text_size(surface::css(14.)).line_height(surface::css(17.))
                    .child(dpi_line("CURRENT_DPI",self.state.current_dpi(),false))
                    .child(dpi_line("NEW_DPI",self.state.new_dpi(),true)))
                .child(BaseButton::new("matcher190-reset").accessibility_label(t("RESET_PROFILE")).mx_auto().mt(surface::css(20.)).h(surface::css(17.)).font_weight(FontWeight::EXTRA_LIGHT).text_size(surface::css(14.)).child(div().underline().child(t("RESET_PROFILE")))
                    .on_click(cx.listener(|this,_,_,cx|{cx.stop_propagation();let actions=this.state.reset();this.emit(actions,cx);cx.notify();})));
        }
        content.into_any_element()
    }
    fn warning(&self,cx:&mut Context<Self>)->AnyElement {
        div().absolute().size_full().bg(rgba(0x27272799))
            .child(v_flex().w(surface::css(400.)).min_h(surface::css(120.)).p(surface::css(20.)).gap(surface::css(17.)).bg(rgb(0x111111)).border_1().border_color(rgb(0xfd8611)).rounded(surface::css(3.))
                .child(h_flex().justify_center().text_color(rgb(0xfd8611)).font_family("RazerF5").text_size(surface::css(16.)).line_height(surface::css(15.))
                    .child(img("synapse/mouse-matcher-190/warning.svg").size(surface::css(20.)).mr(surface::css(10.)))
                    .child(t("SENSITIVITY_MATCHER_CALIBRATION_ERROR").to_uppercase()))
                .child(div().text_center().text_color(rgb(0x999999)).text_size(surface::css(14.)).line_height(surface::css(17.)).max_w(surface::css(360.)).child(t("SENSITIVITY_MATCHER_CALIBRATION_ERROR_DESC")))
                .child(h_flex().justify_center().gap(surface::css(5.))
                    .child(action_button("matcher190-error-cancel","CANCEL",0x707070,0xffffff,90.).on_click(cx.listener(|this,_,_,cx|{cx.stop_propagation();let actions=this.state.cancel();this.emit(actions,cx);cx.notify();})))
                    .child(action_button("matcher190-error-retry","RETRY",0x44d62c,0x222222,90.).on_click(cx.listener(|this,_,_,cx|{cx.stop_propagation();let actions=this.state.reset();this.emit(actions,cx);cx.notify();})))))
            .flex().items_center().justify_center().into_any_element()
    }
    fn overlay(&self,window:&mut Window,cx:&mut Context<Self>)->ViewportLayer {
        let viewport=window.viewport_size();
        let body_height=(viewport.height-window.rem_size()*(141./16.)).max(px(0.));
        let steps=h_flex().mx_auto().mt(surface::css(20.)).w(surface::css(283.)).h(surface::css(33.)).justify_between()
            .children((1..=3).flat_map(|step|{
                let mut children=vec![div().size(surface::css(33.)).rounded_full().bg(if self.state.step_active(step){rgb(0x44d62c)}else{rgb(0x707070)}).text_color(rgb(0x000000)).text_size(surface::css(18.)).flex().items_center().justify_center().child(step.to_string()).into_any_element()];
                if step!=3{children.push(dotted(80.).into_any_element());}children
            }));
        let popup=v_flex().w(surface::css(800.)).min_w(surface::css(546.)).h(viewport.height-window.rem_size()*(105./16.)).bg(rgb(0x222222)).rounded_t(surface::css(5.)).overflow_hidden()
            .child(div().relative().h(surface::css(36.)).flex_shrink_0().border_b_1().border_color(rgb(0x5d5d5d)).text_center().font_family("RazerF5").text_size(surface::css(16.)).line_height(surface::css(19.)).py(surface::css(8.5)).text_color(rgb(0x999999)).child(t("SENSITIVITY_MATCHER").to_uppercase())
                .child(BaseButton::new("matcher190-close").accessibility_label(t("CLOSE")).absolute().right_0().top_0().size(surface::css(36.)).flex().items_center().justify_center().hover(|style|style.bg(rgba(0xffffff1a)))
                    .child(img("synapse/mouse-matcher-190/close.svg").size(surface::css(20.)))
                    .on_click(cx.listener(|this,_,_,cx|{cx.stop_propagation();let actions=this.state.close();this.emit(actions,cx);cx.notify();}))))
            .child(div().id("matcher190-popup-body").relative().h(body_height).overflow_y_scroll().track_scroll(&self.body_scroll).source_scrollbar_vertical()
                .child(v_flex().relative().min_h(surface::css(569.)).pt(surface::css(20.)).pb(surface::css(30.)).pl(surface::css(25.))
                    .child(div().text_center().text_size(surface::css(14.)).line_height(surface::css(17.)).mb(surface::css(10.)).child(t("SENSITIVITY_MATCHER_STEP_DESC")))
                    .child(steps).child(self.step_content(window,cx))
                    .child(h_flex().mt(surface::css(120.)).justify_center().gap(surface::css(10.))
                        .child(action_button("matcher190-cancel","CANCEL",0x707070,0xffffff,100.).on_click(cx.listener(|this,_,_,cx|{cx.stop_propagation();let actions=this.state.cancel();this.emit(actions,cx);cx.notify();})))
                        .child(action_button("matcher190-apply","APPLY",0x44d62c,0x222222,100.).disabled(self.state.phase()!=Some("completed")).on_click(cx.listener(|this,_,_,cx|{let actions=this.state.apply();this.emit(actions,cx);cx.notify();}))))
                    .children((self.state.phase()==Some("error")).then(||deferred(self.warning(cx)).with_priority(999)))))
            .on_click(cx.listener(|this,_,_,cx|{let actions=this.state.window_click(false);this.emit(actions,cx);cx.notify();}));
        let backdrop=div().w(viewport.width).h(viewport.height).bg(rgba(0x00000080)).relative()
            .child(img("synapse/mouse-matcher-190/glow.svg").absolute().bottom_0().w_full())
            .on_click(cx.listener(|this,_,_,cx|{let actions=this.state.window_click(false);this.emit(actions,cx);cx.notify();}));
        ViewportLayer{backdrop:backdrop.into_any_element(),popup:popup.into_any_element()}
    }
}
impl Render for MouseDpiMatcher {
    fn render(&mut self,window:&mut Window,cx:&mut Context<Self>)->impl IntoElement {
        let bounds=self.calibrate_bounds.clone();
        surface::panel_with_control(t("SENSITIVITY_MATCHER"),surface::help_control("matcher190-help",t("SENSITIVITY_MATCHER_TOOLTIP")),cx)
            .child(t("SENSITIVITY_MATCHER_DESC"))
            .child(v_flex().relative()
                .child(BaseButton::new("matcher190-calibrate").accessibility_label(t("CALIBRATE")).mt(surface::css(5.)).h(surface::css(28.)).min_w(surface::css(90.)).px(surface::css(16.)).py(surface::css(6.5)).rounded(surface::css(3.)).border_1().border_color(rgba(0x0000004d)).bg(rgb(0x44d62c)).text_color(rgb(0x212121)).text_size(surface::css(12.)).child(t("CALIBRATE"))
                    .on_prepaint(move |rect,_,_|bounds.set(rect))
                    .on_click(cx.listener(|this,_,_,cx|{cx.stop_propagation();this.state.open();cx.notify();})))
                .child(div().mt(surface::css(20.)).w_full().child(t("SENSITIVITY_MATCHER_PROFILE_DESC")))
                .child(div().ml(surface::css(-16.)).mt(surface::css(10.)).child(self.profile_bar(window,cx)))
                .children(self.state.delete_all_open().then(||deferred(self.delete_all_popup(cx)).with_priority(112))))
            .children(self.state.popup().then(||deferred(self.overlay(window,cx)).with_priority(106)))
            .on_click(cx.listener(|this,_,_,cx|{let actions=this.state.window_click(false);this.emit(actions,cx);cx.notify();}))
    }
}
fn menu_button(id:&'static str,key:&str)->BaseButton {
    BaseButton::new(id).accessibility_label(t(key)).h(surface::css(19.)).px(surface::css(6.)).py(surface::css(2.5)).text_size(surface::css(12.)).line_height(surface::css(17.)).text_color(rgb(0x999999)).bg(rgb(0x000000)).hover(|style|style.bg(rgb(0x1a1a1a))).child(t(key))
}
fn action_button(id:&'static str,key:&str,background:u32,foreground:u32,width:f32)->BaseButton {
    BaseButton::new(id).accessibility_label(t(key)).h(surface::css(27.)).min_w(surface::css(width)).px(surface::css(5.)).py(surface::css(4.)).text_size(surface::css(12.)).line_height(surface::css(17.)).text_center().bg(rgb(background)).text_color(rgb(foreground)).hover(|style|style.opacity(0.8)).child(t(key).to_uppercase())
}
fn dotted(width:f32)->Div {
    h_flex().w(surface::css(width)).h(surface::css(2.)).justify_between()
        .children((0..(width/4.) as usize).map(|_|div().size(surface::css(2.)).rounded_full().bg(rgb(0x707070))))
}
fn dpi_line(key:&str,value:&Value,new:bool)->Div {
    h_flex().justify_between().gap(surface::css(10.)).child(t(key))
        .child(div().text_color(if new{rgb(0x44d62c)}else{rgb(0xcccccc)}).child(match value{Value::Null=>String::new(),Value::String(value)=>value.clone(),_=>value.to_string()}))
}
struct ViewportLayer {backdrop:AnyElement,popup:AnyElement}
impl IntoElement for ViewportLayer {type Element=Self;fn into_element(self)->Self{self}}
impl Element for ViewportLayer {
    type RequestLayoutState=LayoutId;type PrepaintState=();
    fn id(&self)->Option<ElementId>{None}
    fn source_location(&self)->Option<&'static core::panic::Location<'static>>{None}
    fn request_layout(&mut self,_:Option<&GlobalElementId>,_:Option<&InspectorElementId>,window:&mut Window,cx:&mut App)->(LayoutId,LayoutId){let backdrop=self.backdrop.request_layout(window,cx);let popup=self.popup.request_layout(window,cx);(window.request_layout(Style{position:Position::Absolute,..Style::default()},[backdrop,popup],cx),popup)}
    fn prepaint(&mut self,_:Option<&GlobalElementId>,_:Option<&InspectorElementId>,bounds:Bounds<Pixels>,popup:&mut LayoutId,window:&mut Window,cx:&mut App){let size=window.layout_bounds(*popup).size;let viewport=window.viewport_size();window.with_element_offset(point(px(0.),px(0.))-bounds.origin,|window|self.backdrop.prepaint(window,cx));window.with_element_offset(point((viewport.width-size.width)/2.,window.rem_size()*(105./16.))-bounds.origin,|window|self.popup.prepaint(window,cx));}
    fn paint(&mut self,_:Option<&GlobalElementId>,_:Option<&InspectorElementId>,_:Bounds<Pixels>,_:&mut LayoutId,_:&mut (),window:&mut Window,cx:&mut App){self.backdrop.paint(window,cx);self.popup.paint(window,cx);}
}
