//! Mapped regressions for recipient highlighting and cached message rows.
use super::*;
use actions::tests::{pump,until};
use adw::prelude::*;

pub(crate) fn exercise(context:&gtk::glib::MainContext){
    exercise_replies(context);
    let own=Uuid::new_v4();let peer=Uuid::new_v4();let channel_id=Uuid::new_v4();
    let token=format!("@mention(<@{own}>)");let other=format!("@mention(<@{peer}>)");
    let date=chrono::DateTime::parse_from_rfc3339("2026-10-13T12:00:00Z").unwrap();
    let bodies=["Uma mensagem normal.".into(),format!("Olá {token}, tudo bem?"),"Olá @ISpeakHue!".into(),"Bom dia @everyone!".into(),format!("Olá {other}!"),format!("Exemplo: `{token}`"),"https://example.test/@ISpeakHue".into(),format!("Minha própria mensagem: {token}"),"Mais uma mensagem normal.".into()];
    let messages:Vec<Message>=bodies.into_iter().enumerate().map(|(i,content)|serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel_id,"author_id":if i==7{own}else{peer},"content":content,"created_at":date+chrono::Duration::seconds(i as i64)})).unwrap()).collect();
    let ids:Vec<_>=messages.iter().map(|m|m.id).collect();
    let channel:Channel=serde_json::from_value(serde_json::json!({"id":channel_id,"name":"menções","type":"text","created_at":date})).unwrap();
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(channel.clone()),messages,user_id:Some(own),..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:own,access:crate::models::Access::resolve(own,Some(own),&[],&[],&[])});
    let window=adw::Window::builder().default_width(950).default_height(700).content(chat.widget()).build();window.present();
    let row=|id|chat.model().rendered.get(&format!("message-{id}")).map(|(_,row)|row.clone());
    let highlighted=|id|row(id).is_some_and(|r|r.has_css_class("papo-message-mentioned"));
    until(context,||row(ids[8]).is_some_and(|r|r.is_mapped()));
    assert!(highlighted(ids[1]),"ID mentions work before member metadata loads");assert!(!highlighted(ids[2]));
    let member=|id,username,nickname|serde_json::from_value(serde_json::json!({"id":id,"username":username,"nickname":nickname,"created_at":date})).unwrap();
    let own_user:UserSummary=member(own,"alice","ISpeakHue");let peer_user:UserSummary=member(peer,"bob","Bob");
    chat.emit(ChatMsg::SetUsers(vec![own_user.clone(),peer_user.clone()]));until(context,||highlighted(ids[2]));
    for (i,id) in ids.iter().enumerate(){assert_eq!(highlighted(*id),[1,2,3].contains(&i),"wrong mention background on row {i}");}
    let style=adw::StyleManager::default();let previous=style.color_scheme();
    for (scheme,name) in [(adw::ColorScheme::ForceLight,"mention-background-light"),(adw::ColorScheme::ForceDark,"mention-background-dark")]{style.set_color_scheme(scheme);crate::ui::main_window::layout::tests::preview(&window,name,context);}style.set_color_scheme(previous);
    let original=row(ids[1]).unwrap();let live=Uuid::new_v4();
    chat.emit(ChatMsg::AddMessage(serde_json::from_value(serde_json::json!({"id":live,"channel_id":channel_id,"author_id":peer,"content":token,"created_at":date+chrono::Duration::minutes(1)})).unwrap()));until(context,||highlighted(live));
    assert_eq!(row(ids[1]).unwrap(),original,"incremental updates reuse existing highlighted rows");
    chat.emit(ChatMsg::Action(ActionMsg::Navigate(ids[1])));until(context,||row(ids[1]).unwrap().has_css_class("papo-message-highlight"));
    let epoch=chat.model().actions.epoch;let generation=chat.model().highlight_generation;
    chat.emit(ChatMsg::ExpireHighlight{epoch,id:ids[1],generation});until(context,||!row(ids[1]).unwrap().has_css_class("papo-message-highlight"));
    assert!(highlighted(ids[1]));assert_eq!(row(ids[1]).unwrap(),original,"the persistent mention background survives navigation highlight expiry");
    chat.emit(ChatMsg::ApplyChange(Change::Edit(live,"Menção removida.".into(),Some(date.into()))));until(context,||!highlighted(live));
    let mut renamed=own_user.clone();renamed.nickname=Some("Outro nome".into());chat.emit(ChatMsg::SetUsers(vec![renamed,peer_user.clone()]));until(context,||!highlighted(ids[2]));assert!(highlighted(ids[1]));
    let mut duplicate=peer_user.clone();duplicate.nickname=Some("ISpeakHue".into());chat.emit(ChatMsg::SetUsers(vec![own_user.clone(),duplicate]));pump(context);assert!(!highlighted(ids[2]),"ambiguous nicknames cannot highlight the wrong recipient");
    chat.emit(ChatMsg::SetUsers(vec![own_user,peer_user]));until(context,||highlighted(ids[2]));
    let other_channel:Channel=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"name":"outro","type":"text","created_at":date})).unwrap();
    chat.emit(ChatMsg::SetChannel(other_channel));pump(context);chat.emit(ChatMsg::SetChannel(channel));until(context,||highlighted(ids[1])&&highlighted(ids[2]));assert!(!highlighted(live),"cached edits cannot restore stale mention backgrounds");
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);pump(context);
}

/// Replies share the persistent mention background and cached-row invalidation.
fn exercise_replies(context:&gtk::glib::MainContext){
    let own=Uuid::new_v4();let peer=Uuid::new_v4();let channel_id=Uuid::new_v4();let missing=Uuid::new_v4();
    let date=chrono::DateTime::parse_from_rfc3339("2026-10-15T12:00:00Z").unwrap().with_timezone(&chrono::Utc);
    let make=|author,reply,second|serde_json::from_value::<Message>(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel_id,"author_id":author,"reply_to":reply,"content":"Resposta sem menção explícita.","created_at":date+chrono::Duration::seconds(second)})).unwrap();
    let mine=make(Some(own),None,0);let theirs=make(Some(peer),None,1);
    let answer=make(Some(peer),Some(mine.id),2);let unrelated=make(Some(peer),Some(theirs.id),3);let self_reply=make(Some(own),Some(mine.id),4);
    let unknown=make(Some(peer),Some(missing),5);let webhook=make(None,Some(mine.id),6);
    let channel:Channel=serde_json::from_value(serde_json::json!({"id":channel_id,"name":"respostas","created_at":date})).unwrap();
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(channel.clone()),messages:vec![mine.clone(),theirs,answer.clone(),unrelated.clone(),self_reply.clone(),unknown.clone(),webhook.clone()],user_id:Some(own),..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:own,access:crate::models::Access::direct()});
    let host=adw::Window::builder().default_width(900).default_height(700).content(chat.widget()).build();host.present();
    let row=|id|chat.model().rendered.get(&format!("message-{id}")).map(|(_,r)|r.clone());
    let highlighted=|id|row(id).is_some_and(|r|r.has_css_class("papo-message-mentioned"));
    until(context,||highlighted(answer.id)&&highlighted(webhook.id));
    for id in [mine.id,unrelated.id,self_reply.id,unknown.id]{assert!(!highlighted(id),"only incoming replies to the current account are highlighted");}
    // A parent arriving later must invalidate an already-rendered reply.
    let mut late=make(Some(own),None,-1);late.id=missing;chat.emit(ChatMsg::AddMessage(late));until(context,||highlighted(unknown.id));
    let stable=row(answer.id).unwrap();let live=make(Some(peer),Some(mine.id),7);chat.emit(ChatMsg::AddMessage(live.clone()));until(context,||highlighted(live.id));assert_eq!(row(answer.id).unwrap(),stable);
    chat.emit(ChatMsg::ApplyChange(Change::Edit(live.id,"Resposta editada.".into(),Some(date))));pump(context);assert!(highlighted(live.id),"text edits cannot remove the reply highlight");
    chat.emit(ChatMsg::Action(ActionMsg::Navigate(answer.id)));until(context,||row(answer.id).is_some_and(|r|r.has_css_class("papo-message-highlight")));
    let model=chat.model();let epoch=model.actions.epoch;let generation=model.highlight_generation;drop(model);
    chat.emit(ChatMsg::ExpireHighlight{epoch,id:answer.id,generation});until(context,||row(answer.id).is_some_and(|r|!r.has_css_class("papo-message-highlight")));assert!(highlighted(answer.id));
    let mut other=channel.clone();other.id=Uuid::new_v4();chat.emit(ChatMsg::SetChannel(other));pump(context);chat.emit(ChatMsg::SetChannel(channel));until(context,||highlighted(answer.id)&&highlighted(live.id));
    chat.emit(ChatMsg::DeleteMessage(mine.id));until(context,||!highlighted(answer.id)&&!highlighted(live.id));assert!(highlighted(unknown.id),"unrelated reply highlights remain");
    host.set_content(None::<&gtk::Widget>);host.close();drop(chat);pump(context);
}
