//! EN-US: Keep primary application shortcuts native (Command on macOS, Control elsewhere).
//! PT-BR: Mantém os atalhos principais nativos (Command no macOS, Control nos demais sistemas).
use super::{Os,CURRENT};
use gtk::gdk::{Key,ModifierType as M};
fn primary(os:Os,key:&str)->String{format!("{}{key}",if os==Os::Macos{"<Meta>"}else{"<Control>"})}
pub fn accelerator(key:&str)->String{primary(CURRENT.os,key)}
// EN-US: Ignore lock/button state, but reject extra shortcut modifiers (including AltGr).
// PT-BR: Ignora travas/botões, mas rejeita modificadores extras de atalho (incluindo AltGr).
pub fn modifiers(state:M)->M{state&(M::SHIFT_MASK|M::CONTROL_MASK|M::ALT_MASK|M::SUPER_MASK|M::META_MASK|M::HYPER_MASK)}
fn primary_mask(os:Os)->M{if os==Os::Macos{M::META_MASK}else{M::CONTROL_MASK}}
pub fn primary_modifier()->M{primary_mask(CURRENT.os)}
pub fn primary_pressed(state:M)->bool{modifiers(state)==primary_modifier()}
fn paste_for(os:Os,key:Key,state:M)->bool{
    let state=modifiers(state);
    (matches!(key,Key::v|Key::V)&&state==primary_mask(os))||(key==Key::Insert&&state==M::SHIFT_MASK)
}
pub fn paste(key:Key,state:M)->bool{paste_for(CURRENT.os,key,state)}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn command_on_mac_and_control_on_linux_and_windows(){
        assert_eq!(primary(Os::Macos,"comma"),"<Meta>comma");
        for os in [Os::Linux,Os::Windows]{assert_eq!(primary(os,"<Shift>m"),"<Control><Shift>m");}
    }
    #[test]fn paste_preserves_altgr_and_uses_the_platform_primary_modifier(){
        for os in [Os::Linux,Os::Windows,Os::Macos]{
            let primary=primary_mask(os);
            assert!(paste_for(os,Key::v,primary|M::LOCK_MASK));
            assert!(paste_for(os,Key::Insert,M::SHIFT_MASK));
            for extra in [M::ALT_MASK,M::SHIFT_MASK,M::SUPER_MASK]{assert!(!paste_for(os,Key::v,primary|extra));}
            assert!(!paste_for(os,Key::v,if os==Os::Macos{M::CONTROL_MASK}else{M::META_MASK}));
        }
    }
}
