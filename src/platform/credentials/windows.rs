//! EN-US: Bounded UTF-8 credentials and exact-account migration from legacy Windows entries.
//! PT-BR: Credenciais UTF-8 limitadas e migração de entradas Windows antigas vinculada à conta exata.
use anyhow::{anyhow,Result};
const MAGIC:&[u8]=b"PAPO-UTF8\0";
fn entry(service:&str,key:&str)->Result<keyring::Entry>{
    // EN-US: Hash exact bytes; WinCred target names fold case and usernames have a smaller limit.
    // PT-BR: Resume os bytes exatos; nomes WinCred ignoram caixa e usuários têm um limite menor.
    keyring::Entry::new_with_target(&super::super::credential_key::target(service,key),service,"Papo")
        .map_err(|_|anyhow!("Chaveiro indisponível."))
}
fn decode(bytes:Vec<u8>)->Result<String>{
    anyhow::ensure!(bytes.starts_with(MAGIC),"Sessão inválida no chaveiro.");
    String::from_utf8(bytes[MAGIC.len()..].to_vec()).map_err(|_|anyhow!("Sessão inválida no chaveiro."))
}
fn legacy(service:&str,key:&str)->Result<Option<(keyring::Entry,String)>>{
    // EN-US: Such a key could never have been written by keyring's old username-based layout.
    // PT-BR: Uma chave assim não poderia ter sido gravada no layout antigo baseado no usuário.
    if key.len()>513{return Ok(None);}
    let entry=keyring::Entry::new(service,key).map_err(|_|anyhow!("Chaveiro indisponível."))?;
    let bytes=match entry.get_secret(){Ok(bytes)=>bytes,Err(keyring::Error::NoEntry)=>return Ok(None),Err(_)=>return Err(anyhow!("Não foi possível abrir a sessão no chaveiro."))};
    let value=if bytes.starts_with(MAGIC){match decode(bytes){Ok(value)=>value,Err(_)=>return Ok(None)}}else{match entry.get_password(){Ok(value)=>value,Err(keyring::Error::BadEncoding(_))=>return Ok(None),Err(_)=>return Err(anyhow!("Não foi possível abrir a sessão no chaveiro."))}};
    // EN-US: The old lookup folds case. Never use/delete another account's colliding entry.
    // PT-BR: A busca antiga ignora caixa. Nunca usa/remove a entrada conflitante de outra conta.
    let matches=serde_json::from_str::<serde_json::Value>(&value).ok().is_some_and(|v|v["version"]==1&&v["base"].as_str().zip(v["username"].as_str()).is_some_and(|(base,user)|format!("{base}\n{user}")==key));
    Ok(matches.then_some((entry,value)))
}
fn remove(entry:&keyring::Entry)->Result<()>{
    match entry.delete_credential(){Ok(())|Err(keyring::Error::NoEntry)=>Ok(()),Err(_)=>Err(anyhow!("Não foi possível remover a sessão do chaveiro."))}
}
pub fn read(service:&str,key:&str)->Result<Option<String>>{
    match entry(service,key)?.get_secret(){Ok(bytes)=>decode(bytes).map(Some),Err(keyring::Error::NoEntry)=>Ok(legacy(service,key)?.map(|(_,value)|value)),Err(_)=>Err(anyhow!("Não foi possível abrir a sessão no chaveiro."))}
}
pub fn write(service:&str,key:&str,value:Option<&str>)->Result<()>{
    let entry=entry(service,key)?;
    if let Some(value)=value{
        let mut bytes=[MAGIC,value.as_bytes()].concat();let result=entry.set_secret(&bytes);bytes.fill(0);
        result.map_err(|_|anyhow!("Não foi possível salvar a sessão no chaveiro."))?;
        if let Some((old,_))=legacy(service,key)?{remove(&old)?;}
    }else{
        if let Some((old,_))=legacy(service,key)?{remove(&old)?;}
        remove(&entry)?;
    }
    Ok(())
}
