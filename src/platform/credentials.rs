//! EN-US: Native credential-store adapter shared by session persistence and rotation.
//! PT-BR: Adaptador de chaveiro nativo compartilhado pela persistência e rotação das sessões.
#[cfg(not(windows))]use anyhow::{anyhow,Result};
#[cfg(windows)]mod windows;
#[cfg(windows)]pub use windows::{read,write};

#[cfg(not(windows))]
pub fn read(service:&str,key:&str)->Result<Option<String>>{
    let entry=keyring::Entry::new(service,key).map_err(|_|anyhow!("Chaveiro indisponível."))?;
    match entry.get_password(){Ok(value)=>Ok(Some(value)),Err(keyring::Error::NoEntry)=>Ok(None),Err(_)=>Err(anyhow!("Não foi possível abrir a sessão no chaveiro."))}
}
#[cfg(not(windows))]
pub fn write(service:&str,key:&str,value:Option<&str>)->Result<()>{
    let entry=keyring::Entry::new(service,key).map_err(|_|anyhow!("Chaveiro indisponível."))?;
    match value{Some(value)=>entry.set_password(value).map_err(|_|anyhow!("Não foi possível salvar a sessão no chaveiro.")),None=>match entry.delete_credential(){Ok(())|Err(keyring::Error::NoEntry)=>Ok(()),Err(_)=>Err(anyhow!("Não foi possível remover a sessão do chaveiro."))}}
}
