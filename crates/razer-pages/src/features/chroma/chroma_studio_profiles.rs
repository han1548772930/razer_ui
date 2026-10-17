//! Current Studio 8480 profile commands, 7759 local projection producer and
//! 2502 unsaved-change choice. Persisted content belongs to StudioSession.
use super::*;
use gpui_kit::component::{WindowExt as _,h_flex,v_flex};
use studio_dropdown::StudioDropdown;

impl ChromaStudio {
    pub fn set_profiles(&mut self,profiles:Vec<StudioProfileItem>,selected:Option<String>,pending:bool,cx:&mut Context<Self>) {
        self.profiles=profiles;self.selected_profile=selected;self.profiles_pending=pending;cx.notify();
    }
    pub fn next_profile_name(&self)->String {self.profile_name(None)}
    /// The current storage listener dispatches the same selection command as
    /// the profile menu, including its unsaved-change choice (main/8480 O/B).
    pub fn select_external_profile(&mut self,id:String,window:&mut Window,cx:&mut Context<Self>) {
        self.request_profile(StudioProfileCommand::Select {id},window,cx);
    }
    pub fn has_unsaved_changes(&self)->bool {self.document!=self.saved}
    fn profile_name(&self,base:Option<&str>)->String {
        let names=self.profiles.iter().map(|profile|profile.name.as_str()).collect::<Vec<_>>();
        if let Some(base)=base.filter(|base|!base.is_empty()) {
            if !names.contains(&base) {return base.into();}
            let base=studio_layers::suffix_base(base);
            for number in 1..u64::MAX {let name=format!("{base} ({number})");if !names.contains(&name.as_str()) {return name;}}
        } else {
            for number in 1..u64::MAX {let name=format!("{} {number}",label("TEXT_CHROMA_EFFECT"));if !names.contains(&name.as_str()) {return name;}}
        }
        unreachable!("finite Studio profile inventory")
    }
    fn request_profile(&mut self,command:StudioProfileCommand,window:&mut Window,cx:&mut Context<Self>) {
        if self.profiles_pending {return;}
        if matches!(&command,StudioProfileCommand::Select {id} if self.selected_profile.as_ref()==Some(id)) {return;}
        if self.has_unsaved_changes()&&matches!(command,StudioProfileCommand::Add {..}|StudioProfileCommand::Select {..}|StudioProfileCommand::Duplicate {..}) {
            let owner=cx.entity().downgrade();
            window.open_dialog(cx,move |dialog,_,_| {
                let save_owner=owner.clone();let discard_owner=owner.clone();
                let save_command=command.clone();let discard_command=command.clone();
                dialog.title(razer_assets::resource_scope(label("SAVE_CHROMA_EFFECT_TITLE")))
                    .overlay_closable(true)
                    .child(razer_assets::resource_scope(v_flex().gap(surface::css(14.)).child(label("SAVE_CHROMA_EFFECT_DESC_1")).child(label("SAVE_CHROMA_EFFECT_DESC_2"))))
                    .footer(razer_assets::resource_scope(h_flex().gap(surface::css(10.))
                        .child(Button::new("studio-profile-save-switch").label(label("SAVE")).on_click(move |_,window,cx|{
                            if let Some(owner)=save_owner.upgrade() {owner.update(cx,|page,cx| {cx.emit(ChromaStudioEvent::Save(page.snapshot()));cx.emit(ChromaStudioEvent::Profile(save_command.clone()));});}
                            window.close_dialog(cx);
                        }))
                        .child(Button::new("studio-profile-discard-switch").label(label("DONT_SAVE")).on_click(move |_,window,cx|{
                            if let Some(owner)=discard_owner.upgrade() {owner.update(cx,|_,cx|cx.emit(ChromaStudioEvent::Profile(discard_command.clone())));}
                            window.close_dialog(cx);
                        }))))
            });
        } else {cx.emit(ChromaStudioEvent::Profile(command));}
    }
    fn rename_profile(&mut self,window:&mut Window,cx:&mut Context<Self>) {
        let Some(profile)=self.profiles.iter().find(|profile|Some(&profile.id)==self.selected_profile.as_ref()) else{return;};
        let id=profile.id.clone();let name=profile.name.clone();
        let input=cx.new(|cx|{let mut input=InputState::new(window,cx);input.set_value(name,window,cx);input});
        self.profile_rename_subscription=Some(cx.subscribe(&input,|page,_,event,cx|{
            if matches!(event,InputEvent::Blur|InputEvent::PressEnter {..}) {
                let Some((id,input))=page.profile_rename.take() else{return;};
                page.profile_rename_subscription=None;
                let value=input.read(cx).value();let mut length=0;
                let name=value.trim().chars().take_while(|character|{length+=character.len_utf16();length<=32}).collect::<String>();
                if !name.is_empty()&&!page.profiles.iter().any(|profile|profile.name==name) {cx.emit(ChromaStudioEvent::Profile(StudioProfileCommand::Rename {id,name}));}
                cx.notify();
            }
        }));
        input.update(cx,|input,cx|{input.focus(window,cx);input.select_all(window,cx);});
        self.profile_rename=Some((id,input));cx.notify();
    }
    pub(super) fn profile_header(&mut self,window:&mut Window,cx:&mut Context<Self>)->AnyElement {
        let mut list=v_flex();
        for (position,profile) in self.profiles.iter().enumerate() {
            let id=profile.id.clone();let menu=self.profile_dropdown.clone();
            list=list.child(button(ElementId::Name(format!("studio-profile-{}",profile.id).into()),"CHANGE_EFFECT")
                .w_full().px(surface::css(10.)).py(surface::css(5.)).justify_start()
                .when(Some(&profile.id)==self.selected_profile.as_ref(),|button|button.text_color(Colors::selected()))
                .child(format!("{}. {}",position+1,profile.name))
                .on_click(cx.listener(move |page,_,window,cx|{menu.update(cx,|menu,cx|menu.set_open(false,cx));page.request_profile(StudioProfileCommand::Select {id:id.clone()},window,cx);})));}
        let selected=self.profiles.iter().enumerate().find(|(_,profile)|Some(&profile.id)==self.selected_profile.as_ref());
        let title=selected.map(|(position,profile)|format!("{}. {}",position+1,profile.name)).unwrap_or_default();
        let selector=if let Some((_,input))=&self.profile_rename {
            div().w(surface::css(180.)).child(Input::new(input)).into_any_element()
        }else{
            StudioDropdown::new("studio-profiles",&self.profile_dropdown,!self.profiles_pending&&self.profiles.len()>1,
                button("studio-profiles-trigger","CHANGE_EFFECT").w(surface::css(180.)).h(surface::css(27.)).bg(Colors::black()).border_1().border_color(Colors::input_border()).child(div().flex_1().truncate().child(title)).child(studio_dropdown::arrow(self.profile_dropdown.read(cx).is_open(),"studio-profiles-arrow".into(),window,cx)),
                list.into_any_element(),180.,0.).into_any_element()
        };
        let mut actions=v_flex();
        if self.profile_delete_confirm {
            let menu=self.profile_actions.clone();let id=self.selected_profile.clone();
            actions=actions.p(surface::css(10.)).gap(surface::css(10.)).child(label("DELETE_CHROMA_EFFECT")).child(label("DELETE_CHROMA_EFFECT_DESC"))
                .child(button("studio-profile-delete-confirm","DELETE").child(label("DELETE")).on_click(cx.listener(move |page,_,window,cx|{
                    page.profile_delete_confirm=false;menu.update(cx,|menu,cx|menu.set_open(false,cx));
                    if let Some(id)=&id {page.request_profile(StudioProfileCommand::Delete {id:id.clone()},window,cx);}
                })));
        } else {
            for key in ["ADD","RENAME","DUPLICATE","DELETE"] {
                let menu=self.profile_actions.clone();
                let disabled=self.profiles_pending||(key!="ADD"&&self.selected_profile.is_none())||(key=="DELETE"&&self.profiles.len()<=1);
                actions=actions.child(button(ElementId::Name(format!("studio-profile-action-{key}").into()),key).w_full().px(surface::css(10.)).py(surface::css(5.)).justify_start().disabled(disabled).child(label(key))
                    .on_click(cx.listener(move |page,_,window,cx|{
                        if key=="DELETE" {page.profile_delete_confirm=true;cx.notify();return;}
                        menu.update(cx,|menu,cx|menu.set_open(false,cx));
                        if key=="RENAME" {page.rename_profile(window,cx);return;}
                        let command=if key=="ADD" {Some(StudioProfileCommand::Add {name:page.profile_name(None)})}else{page.selected_profile.as_ref().and_then(|id|page.profiles.iter().find(|profile|profile.id==*id)).map(|profile|StudioProfileCommand::Duplicate {id:profile.id.clone(),name:page.profile_name(Some(&profile.name))})};
                        if let Some(command)=command {page.request_profile(command,window,cx);}
                    })));
            }
        }
        h_flex().gap(surface::css(10.)).child(selector).child(StudioDropdown::new("studio-profile-actions",&self.profile_actions,!self.profiles_pending,
            button("studio-profile-actions-trigger","CHANGE_EFFECT").w(surface::css(27.)).h(surface::css(27.)).bg(Colors::black()).border_1().border_color(Colors::input_border()).child("…"),
            actions.into_any_element(),170.,0.)).into_any_element()
    }
}
