//! Current756 UI55714 ImportExportModal. File selection parses a local
//! snapshot; the workspace owns subsequent persistence and device publication.
use super::*;
use gpui_kit::base::{Button as BaseButton, Dialog, DialogPopup};
use gpui_kit::component::checkbox::Checkbox;
use razer_i18n as i18n;
use serde::Deserialize;
use std::{collections::BTreeSet, path::PathBuf, sync::OnceLock};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ProfileTransferMode { Import, Export }
#[derive(Deserialize)] struct TransferSpec {width:f32,height:f32,top:f32}
fn spec(pid:u32)->&'static TransferSpec {
    #[derive(Deserialize)] struct ProductSpec {product_id:u32,import_export:TransferSpec}
    #[derive(Deserialize)] struct Data {products:Vec<ProductSpec>}
    static DATA:OnceLock<Data>=OnceLock::new();
    &DATA.get_or_init(||serde_json::from_str(include_str!("../../profiles/source_profile_menu_data.json"))
        .expect("audited current profile transfer geometry")).products.iter().find(|p|p.product_id==pid)
        .expect("mounted current profile transfer").import_export
}
fn text(key:&str)->String {i18n::t(key)}
struct MacroRow {guid:String,name:String,selected:bool}
struct ProfileRow {profile:Value,selected:bool,macros:Vec<MacroRow>}
fn macro_refs(value:&Value,result:&mut Vec<(String,String)>) {
    if let Some(group)=value.get("macroGroup") {
        if let Some(guid)=group["guid"].as_str() {
            if !result.iter().any(|(old,_)|old==guid) {result.push((guid.into(),group["name"].as_str().unwrap_or("").into()));}
        }
    }
    if let Some(stages)=value["mapping"].as_array() {for stage in stages {macro_refs(stage,result);}}
}
fn row(profile:Value,selected:bool,macros:&[Value],importing:bool)->ProfileRow {
    let mut refs=Vec::new();
    for mapping in profile["mappings"].as_array().into_iter().flatten() {macro_refs(mapping,&mut refs);}
    for keymap in profile["keymaps"].as_array().into_iter().flatten() {
        for mapping in keymap["mappings"].as_array().into_iter().flatten() {macro_refs(mapping,&mut refs);}
    }
    let macros=refs.into_iter().filter_map(|(guid,name)| {
        let found=macros.iter().find(|mac|mac["guid"]==guid);
        if importing&&found.is_none() {return None;}
        Some(MacroRow{name:found.and_then(|mac|mac["name"].as_str()).unwrap_or(&name).into(),guid,selected:true})
    }).collect();ProfileRow{profile,selected,macros}
}
pub(super) struct SourceProfileTransfer {
    pid:u32,mode:ProfileTransferMode,target:Value,profiles:Vec<ProfileRow>,macros:Vec<Value>,
    deselect_all:bool,device_name:String,warning:bool,path:Option<PathBuf>,picker_pending:bool,
    picker_generation:u64,picker_task:Option<Task<()>>,error:Option<String>,focus:FocusHandle,closed:bool,
}
pub(super) enum SourceProfileTransferEvent {
    Close,Import{rows:Vec<Value>,macros:Vec<Value>},Export{profiles:Vec<Value>,macros:Vec<Value>},
}
impl EventEmitter<SourceProfileTransferEvent> for SourceProfileTransfer {}
impl SourceProfileTransfer {
    pub(super) fn new(pid:u32,mode:ProfileTransferMode,target:Value,profiles:Vec<Value>,macros:Vec<Value>,
        active:String,_window:&mut Window,cx:&mut Context<Self>)->Self {
        let factory=target["presetProfiles"].as_array().into_iter().flatten().find(|p|p["isFactoryPreset"]==true)
            .and_then(|p|p["guid"].as_str()).unwrap_or("");
        let mut profiles=if mode==ProfileTransferMode::Export {
            profiles.into_iter().filter(|p|p["guid"]!=factory).collect::<Vec<_>>()
        }else{Vec::new()};
        if profiles.iter().any(|p|p["name"]=="General")&&profiles.iter().any(|p|p["name"]=="Factory Default") {
            profiles.retain(|p|p["name"]!="Factory Default");
        }
        let profiles=profiles.into_iter().map(|profile| {
            let selected=pid==3886||profile["guid"]==active;row(profile,selected,&macros,false)
        }).collect();
        let device_name=target["deviceName"].as_str().unwrap_or("").into();
        Self{pid,mode,target,profiles,macros,deselect_all:false,device_name,warning:false,path:None,
            picker_pending:false,picker_generation:0,picker_task:None,error:None,focus:cx.focus_handle(),closed:false}
    }
    pub(super) fn focus(&self,window:&mut Window,cx:&mut App) {self.focus.focus(window,cx);}
    fn close(&mut self,cx:&mut Context<Self>) {
        if self.closed{return;}self.closed=true;self.picker_generation=self.picker_generation.wrapping_add(1);
        self.picker_task=None;cx.emit(SourceProfileTransferEvent::Close);
    }
    fn browse(&mut self,cx:&mut Context<Self>) {
        if self.mode!=ProfileTransferMode::Import||self.picker_pending{return;}
        self.picker_pending=true;self.error=None;self.picker_generation=self.picker_generation.wrapping_add(1);
        let generation=self.picker_generation;let target=self.target.clone();
        let picker=cx.prompt_for_paths(PathPromptOptions{files:true,directories:false,multiple:false,
            prompt:Some(format!("{} (.synapse4)",text("BROWSE_FOR_PROFILES")).into())});
        self.picker_task=Some(cx.spawn(async move|owner,cx|{
            let result=match picker.await {
                Ok(Ok(Some(paths)))=>paths.into_iter().next().map(Ok),Ok(Ok(None))=>None,
                _=>Some(Err("Could not open the profile file picker".to_owned())),
            };
            let prepared=owner.update(cx,|this,cx|{
                if this.closed||this.picker_generation!=generation{return None;}
                if let Some(Ok(path))=&result{this.path=Some(path.clone());}cx.notify();result
            });
            let loaded=match prepared {
                Ok(Some(Ok(path)))=>Some(cx.background_executor().spawn(async move{
                    std::fs::read(path).map_err(|e|e.to_string()).and_then(|bytes|
                        razer_device::profile_transfer::decode(&bytes,&target).map_err(|e|e.to_string()))
                }).await),Ok(Some(Err(error)))=>Some(Err(error)),Ok(None)=>None,Err(_)=>return,
            };
            let _=owner.update(cx,|this,cx|{
                if this.closed||this.picker_generation!=generation{return;}
                this.picker_pending=false;this.picker_task=None;
                if let Some(loaded)=loaded{match loaded{
                    Ok(file)=>{
                        this.profiles=file.profiles().iter().cloned().map(|p|row(p,true,file.macros(),true)).collect();
                        this.macros=file.macros().iter().map(razer_device::profile_transfer::decode_xml_entities).collect();
                        this.device_name=file.device_name().into();this.warning=file.has_warning();this.error=None;
                    }
                    Err(error)=>{this.profiles.clear();this.macros.clear();this.warning=false;this.error=Some(error);}
                }}cx.notify();
            });
        }));cx.notify();
    }
    fn submit(&mut self,cx:&mut Context<Self>) {
        if self.closed||self.picker_pending||!self.profiles.iter().any(|r|r.selected){return;}
        if self.mode==ProfileTransferMode::Import {
            let rows=self.profiles.iter().filter(|r|r.selected).map(|r|serde_json::json!({"profile":r.profile,
                "hasWarning":self.warning,"listItems":r.macros.iter().map(|m|serde_json::json!({"guid":m.guid,
                    "text":m.name,"isMacroSelected":m.selected})).collect::<Vec<_>>() })).collect();
            cx.emit(SourceProfileTransferEvent::Import{rows,macros:self.macros.clone()});
        }else{
            let mut required=BTreeSet::new();let mut macros=Vec::new();
            for r in self.profiles.iter().filter(|r|r.selected){for mac in r.macros.iter().filter(|m|m.selected){
                if !required.insert(mac.guid.clone()){continue;}
                let Some(value)=self.macros.iter().find(|v|v["guid"]==mac.guid)else{
                    self.error=Some(format!("The referenced macro {} is absent from local storage",mac.name));cx.notify();return;
                };macros.push(value.clone());
            }}cx.emit(SourceProfileTransferEvent::Export{profiles:self.profiles.iter().filter(|r|r.selected)
                .map(|r|r.profile.clone()).collect(),macros});
        }self.close(cx);
    }
    fn header(&self,cx:&mut Context<Self>)->impl IntoElement {
        h_flex().relative().w_full().h(surface::css(36.)).flex_shrink_0().justify_center().bg(rgb(0x222222))
            .border_b_1().border_color(rgb(0x515151)).rounded_t(surface::css(5.)).text_color(rgb(0x999999))
            .child(text(if self.mode==ProfileTransferMode::Import{"TEXT_IMPORT_PROFILES"}else{"TEXT_EXPORT_PROFILES"}).to_uppercase())
            .child(BaseButton::new("source-transfer-close").absolute().right(surface::css(5.)).size(surface::css(20.))
                .accessibility_label(text("CLOSE")).focus_visible(|s|s.border_1().border_color(rgb(0x44d62c)))
                .child(img("synapse/profiles-close.svg").size_full()).on_click(cx.listener(|this,_,_,cx|this.close(cx))))
    }
    fn browse_row(&self,cx:&mut Context<Self>)->impl IntoElement {
        let filename=self.path.as_ref().and_then(|p|p.file_name()).map(|n|n.to_string_lossy().into_owned()).unwrap_or_default();
        h_flex().w_full().h(surface::css(47.)).flex_shrink_0().bg(rgb(0x222222)).pl(surface::css(100.))
            .child(text("BROWSE_FOR_PROFILES")).child(BaseButton::new("source-transfer-browse").ml(surface::css(10.))
                .w(surface::css(250.)).h(surface::css(27.)).flex_shrink_0().pl(surface::css(6.)).pr(surface::css(4.))
                .border_1().border_color(rgb(0x5d5d5d)).bg(rgb(0x111111)).flex().items_center().disabled(self.picker_pending)
                .accessibility_label(text("BROWSE_FOR_PROFILES")).hover(|s|s.border_color(rgb(0x44c62d)))
                .focus_visible(|s|s.border_color(rgb(0x44c62d))).child(div().flex_1().min_w_0().truncate().child(filename))
                .child(img("synapse/source-transfer-folder.svg").size(surface::css(20.)))
                .on_click(cx.listener(|this,_,_,cx|this.browse(cx))))
    }
    fn body(&self,cx:&mut Context<Self>)->AnyElement {
        let mut body=v_flex().flex_1().min_h_0();
        if !self.profiles.is_empty(){
            body=body.child(BaseButton::new("source-transfer-select-all").w_full().h(surface::css(30.)).pr(surface::css(27.))
                .flex().justify_end().text_color(rgb(0x44d62c)).child(text(if self.deselect_all{"TEXT_SELECT_ALL_PROFILES"}else{"TEXT_DESELECT_ALL_PROFILES"}))
                .on_click(cx.listener(|this,_,_,cx|{
                    for row in &mut this.profiles{row.selected=this.deselect_all;}this.deselect_all=!this.deselect_all;cx.notify();
                })));
            body=body.child(v_flex().id("source-transfer-profile-list").flex_1().min_h_0().px(surface::css(27.))
                .scrollable_y().child(div().mb(surface::css(6.)).child(self.device_name.clone()))
                .children(self.profiles.iter().enumerate().map(|(index,row)|{
                    v_flex().mb(surface::css(10.)).child(Checkbox::new(("source-transfer-profile",index))
                        .h(surface::css(20.)).flex_shrink_0().label(row.profile["name"].as_str().unwrap_or("").to_owned()).checked(row.selected)
                        .on_click(cx.listener(move|this,checked,_,cx|{
                            if let Some(row)=this.profiles.get_mut(index){row.selected=*checked;if *checked{for mac in &mut row.macros{mac.selected=true;}}}cx.notify();
                        })))
                        .when(row.selected,|v|v.child(v_flex().mt(surface::css(5.)).mb(surface::css(5.))
                            .children(row.macros.iter().enumerate().map(|(macro_index,mac)|{
                                h_flex().pl(surface::css(10.)).child(div().w(surface::css(30.)).text_size(surface::css(11.))
                                    .child(if macro_index==0{"Macro"}else{""}))
                                    .child(Checkbox::new(("source-transfer-macro",index*4096+macro_index)).h(surface::css(20.))
                                        .label(mac.name.clone()).checked(mac.selected).on_click(cx.listener(move|this,checked,_,cx|{
                                            if let Some(mac)=this.profiles.get_mut(index).and_then(|r|r.macros.get_mut(macro_index)){mac.selected=*checked;}cx.notify();
                                        })))
                            }))))
                })));
        }
        if let Some(error)=&self.error{body=body.child(div().px(surface::css(27.)).py(surface::css(10.)).text_color(rgb(0xfd4949)).child(error.clone()));}
        body.into_any_element()
    }
    fn footer(&self,cx:&mut Context<Self>)->impl IntoElement {
        let disabled=self.picker_pending||!self.profiles.iter().any(|r|r.selected);
        v_flex().w_full().flex_shrink_0().items_center().py(surface::css(16.)).border_t_1().border_color(rgb(0x515151))
            .rounded_b(surface::css(5.)).bg(rgb(0x222222))
            .when(self.mode==ProfileTransferMode::Export,|v|v.child(div().px(surface::css(10.)).mb(surface::css(10.))
                .text_center().line_height(surface::css(16.)).child(text("IMPORT_EXPORT_MODAL_WILL_NOT_IMPORT"))))
            .child(h_flex().h(surface::css(27.)).gap(surface::css(10.))
                .child(Self::button("source-transfer-cancel","CANCEL",false,false).on_click(cx.listener(|this,_,_,cx|this.close(cx))))
                .child(Self::button("source-transfer-submit",if self.mode==ProfileTransferMode::Import{"IMPORT"}else{"EXPORT"},true,disabled)
                    .on_click(cx.listener(|this,_,_,cx|this.submit(cx)))))
            .when(self.warning,|v|v.child(div().mt(surface::css(10.)).px(surface::css(20.)).text_color(rgb(0xffaa00))
                .child(text("IMPORT_EXPORT_MODAL_TIP"))))
    }
    fn button(id:&'static str,key:&str,primary:bool,disabled:bool)->BaseButton {
        BaseButton::new(id).h_full().px(surface::css(24.)).border_1().border_color(rgba(0x0000004d)).rounded(surface::css(3.))
            .flex().items_center().justify_center().text_size(surface::css(12.)).line_height(surface::css(14.))
            .bg(rgb(if primary{0x44d62c}else{0x707070})).text_color(rgb(if primary{0}else{0xffffff}))
            .disabled(disabled).when(disabled,|v|v.opacity(0.3)).when(!disabled,|v|v.hover(|s|s.opacity(0.8)).active(|s|s.opacity(0.6)))
            .focus_visible(|s|s.border_color(rgb(0xcccccc))).child(text(key).to_uppercase())
    }
}
impl Render for SourceProfileTransfer {
    fn render(&mut self,window:&mut Window,cx:&mut Context<Self>)->impl IntoElement {
        let g=spec(self.pid);let left=window.viewport_size().width/2.-surface::css(300.).to_pixels(window.rem_size());
        let panel=v_flex().id("source-profile-transfer").test_support().w(surface::css(g.width)).h(surface::css(g.height))
            .border_1().border_color(rgb(0x515151)).rounded(surface::css(5.)).bg(rgb(0x111111)).font_family("Roboto")
            .text_size(surface::css(14.)).line_height(surface::css(17.)).text_color(rgb(0xcccccc)).child(self.header(cx))
            .when(self.mode==ProfileTransferMode::Import,|v|v.child(self.browse_row(cx))).child(self.body(cx)).child(self.footer(cx));
        Dialog::new(cx).layer(3,true).focus_handle(self.focus.clone()).on_cancel(|_,_,_|false).on_ok(|_,_,_|false)
            .close_on_backdrop_press(false).backdrop(div().absolute().inset_0().bg(rgba(0x000000b3)).occlude())
            .popup(DialogPopup::new().absolute().left(left).top(surface::css(g.top)).child(panel))
    }
}
