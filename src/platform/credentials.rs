//! EN-US: Native credential-store adapter shared by session persistence and rotation.
//! PT-BR: Adaptador de chaveiro nativo compartilhado pela persistência e rotação das sessões.
use anyhow::{anyhow,Result};

pub fn read(service:&str,key:&str)->Result<Option<String>>{
    let entry=keyring::Entry::new(service,key).map_err(|_|anyhow!("Chaveiro indisponível."))?;
    // EN-US: UTF-8 avoids halving Windows' credential blob allowance. Read old UTF-16 entries too.
    // PT-BR: UTF-8 evita reduzir à metade o limite do cofre Windows. Lê também entradas UTF-16 antigas.
    #[cfg(windows)] {
        match entry.get_secret(){
            Ok(bytes) if bytes.starts_with(b"PAPO-UTF8\0")=>String::from_utf8(bytes[10..].to_vec()).map(Some).map_err(|_|anyhow!("Sessão inválida no chaveiro.")),
            Ok(_)=>entry.get_password().map(Some).map_err(|_|anyhow!("Não foi possível abrir a sessão no chaveiro.")),
            Err(keyring::Error::NoEntry)=>Ok(None),Err(_)=>Err(anyhow!("Não foi possível abrir a sessão no chaveiro."))
        }
    }
    #[cfg(not(windows))] {match entry.get_password(){Ok(value)=>Ok(Some(value)),Err(keyring::Error::NoEntry)=>Ok(None),Err(_)=>Err(anyhow!("Não foi possível abrir a sessão no chaveiro."))}}
}
pub fn write(service:&str,key:&str,value:Option<&str>)->Result<()>{
    let entry=keyring::Entry::new(service,key).map_err(|_|anyhow!("Chaveiro indisponível."))?;
    match value{Some(value)=>{
        #[cfg(windows)] let result=entry.set_secret(&[b"PAPO-UTF8\0".as_slice(),value.as_bytes()].concat());
        #[cfg(not(windows))] let result=entry.set_password(value);
        result.map_err(|_|anyhow!("Não foi possível salvar a sessão no chaveiro."))
    },None=>match entry.delete_credential(){Ok(())|Err(keyring::Error::NoEntry)=>Ok(()),Err(_)=>Err(anyhow!("Não foi possível remover a sessão do chaveiro."))}}
}
