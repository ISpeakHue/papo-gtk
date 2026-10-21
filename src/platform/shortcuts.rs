//! EN-US: Keep primary application shortcuts native (Command on macOS, Control elsewhere).
//! PT-BR: Mantém os atalhos principais nativos (Command no macOS, Control nos demais sistemas).
use super::{Os,CURRENT};
fn primary(os:Os,key:&str)->String{format!("{}{key}",if os==Os::Macos{"<Meta>"}else{"<Control>"})}
pub fn accelerator(key:&str)->String{primary(CURRENT.os,key)}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn command_on_mac_and_control_on_linux_and_windows(){
        assert_eq!(primary(Os::Macos,"comma"),"<Meta>comma");
        for os in [Os::Linux,Os::Windows]{assert_eq!(primary(os,"<Shift>m"),"<Control><Shift>m");}
    }
}
