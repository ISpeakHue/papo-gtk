//! EN-US: Device-local download and retention choices are independent of animation/motion.
//! PT-BR: Opções locais de download e retenção são independentes de animação/movimento.
use serde::{Serialize,Deserialize};
#[derive(Debug,Clone,Copy,Default,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]pub enum Loading {#[default] Automatic,OnClick}
impl Loading{pub fn index(self)->u32{if self==Self::OnClick{1}else{0}}pub fn from_index(i:u32)->Self{if i==1{Self::OnClick}else{Self::Automatic}}}
#[derive(Debug,Clone,Copy,Default,PartialEq,Eq,Serialize,Deserialize)]
#[serde(default)]pub struct Preferences {pub loading:Loading,pub low_memory:bool}
impl Preferences{pub fn device()->Self{
    #[cfg(test)]if std::env::var_os("PAPO_TEST_CONFIG_DIR").is_none(){return Self::default();}
    crate::config::AppConfig::load().media
}}
#[derive(Debug,Clone,Copy,Default,PartialEq,Eq)]pub struct Usage {pub memory_bytes:u64,pub temporary_bytes:u64,pub disk_bytes:u64}
