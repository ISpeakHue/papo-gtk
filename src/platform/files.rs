//! EN-US: Native file naming and atomic replacement without truncating previous data.
//! PT-BR: Nomes de arquivo nativos e substituição atômica sem truncar dados anteriores.
use anyhow::{Context,Result};
use std::{io::Write,path::{Path,PathBuf}};

fn reserved(name:&str)->bool{
    let stem=name.split('.').next().unwrap_or("").trim_end_matches(' ').to_uppercase();
    matches!(stem.as_str(),"CON"|"PRN"|"AUX"|"NUL"|"CONIN$"|"CONOUT$")
        ||["COM","LPT"].iter().any(|prefix|stem.strip_prefix(prefix).is_some_and(|n|matches!(n,"1"|"2"|"3"|"4"|"5"|"6"|"7"|"8"|"9"|"¹"|"²"|"³")))
}
fn invalid(c:char)->bool{c<' '||matches!(c,'<'|'>'|':'|'"'|'/'|'\\'|'|'|'?'|'*')}
fn windows_name(name:&str)->bool{
    !name.is_empty()&&!name.ends_with(['.',' '])&&!name.chars().any(invalid)&&!reserved(name)&&name.encode_utf16().count()<=255
}

/// EN-US: Only a suggested basename; the user's chosen destination remains authoritative.
/// PT-BR: Apenas um nome sugerido; o destino escolhido pelo usuário continua sendo respeitado.
pub fn suggested_filename(name:&str)->String{
    let name=name.rsplit(['/', '\\']).next().unwrap_or("");
    if !cfg!(windows){return if name.is_empty(){"arquivo".into()}else{name.into()};}
    windows_suggestion(name)
}
fn windows_suggestion(name:&str)->String{
    let cleaned:String=name.chars().map(|c|if invalid(c){'_'}else{c}).collect();
    let mut name=cleaned.trim_end_matches(['.',' ']).to_owned();
    if name.is_empty(){name="arquivo".into();}
    if reserved(&name){name.insert(0,'_');}
    // EN-US: Keep the extension while leaving room for the dialog's overwrite suffix.
    // PT-BR: Preserva a extensão e reserva espaço para o sufixo do diálogo de sobrescrita.
    if name.encode_utf16().count()>200{
        let extension=name.rsplit_once('.').map(|(_,ext)|ext).filter(|ext|ext.encode_utf16().count()<=20).map(|ext|format!(".{ext}")).unwrap_or_default();
        let budget=200-extension.encode_utf16().count();let mut units=0;
        name=name.chars().take_while(|c|{units+=c.len_utf16();units<=budget}).collect();
        name.push_str(&extension);
    }
    name
}

pub fn validate_destination(path:&Path)->Result<()>{
    let name=path.file_name().context("Nome de arquivo inválido.")?;
    if cfg!(windows){anyhow::ensure!(name.to_str().is_some_and(windows_name),"Nome de arquivo inválido no Windows.");}
    Ok(())
}

struct Stage{file:Option<std::fs::File>,path:PathBuf}
impl Drop for Stage{fn drop(&mut self){self.file.take();let _=std::fs::remove_file(&self.path);}}

/// EN-US: Close before replacement/removal so locked Windows files fail without data loss.
/// PT-BR: Fecha antes de substituir/remover para falhar sem perda de dados em arquivos bloqueados.
pub fn atomic_write(destination:&Path,bytes:&[u8])->Result<()>{
    validate_destination(destination)?;
    let parent=destination.parent().context("Pasta de destino inválida.")?;
    let path=parent.join(format!(".papo-config-{}",uuid::Uuid::new_v4()));
    let mut options=std::fs::OpenOptions::new();options.write(true).create_new(true);
    #[cfg(unix)] {use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
    let file=options.open(&path)?;let mut stage=Stage{file:Some(file),path};
    let file=stage.file.as_mut().expect("new stage file");file.write_all(bytes)?;file.sync_all()?;
    stage.file.take();std::fs::rename(&stage.path,destination)?;
    Ok(())
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn windows_suggestions_handle_devices_ads_unicode_and_component_limits(){
        for name in ["CON","con.txt","LPT1.png","COM¹.gif","CONOUT$","NUL .txt","bad:name.png","x?<>\"|*\u{1}.txt",".","..",""]{
            let safe=windows_suggestion(name);assert!(windows_name(&safe),"{name:?} => {safe:?}");
        }
        assert_eq!(windows_suggestion("á é ç.png"),"á é ç.png");
        assert!(windows_suggestion(&("😀".repeat(130)+".png")).ends_with(".png"));
        assert!(windows_name(&windows_suggestion(&("😀".repeat(130)+".png"))));
        for invalid in ["CON.txt","name:stream","name.","name ","COM².txt"]{assert!(!windows_name(invalid));}
    }
    #[test]fn failed_atomic_replace_preserves_destination_and_cleans_stage(){
        let directory=std::env::temp_dir().join(format!("papo-atomic-{}",uuid::Uuid::new_v4()));std::fs::create_dir(&directory).unwrap();
        let dest=directory.join("config.toml");atomic_write(&dest,b"before").unwrap();atomic_write(&dest,b"after").unwrap();assert_eq!(std::fs::read(&dest).unwrap(),b"after");
        assert!(atomic_write(&directory,b"failed").is_err());assert!(directory.is_dir());assert_eq!(std::fs::read_dir(&directory).unwrap().count(),1);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
