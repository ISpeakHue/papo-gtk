//! EN-US: Preserve exact account identity in Windows' case-insensitive credential namespace.
//! PT-BR: Preserva a identidade exata da conta no namespace de credenciais Windows sem distinção de caixa.
use sha2::{Digest,Sha256};
pub(super) fn target(service:&str,key:&str)->String{
    let mut digest=Sha256::new();digest.update((service.len() as u64).to_le_bytes());digest.update(service.as_bytes());digest.update(key.as_bytes());
    format!("br.com.papo.gtk.session.v2:{:x}",digest.finalize())
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn case_paths_unicode_and_service_boundaries_remain_distinct(){
        let service="br.com.papo.gtk.session.v1";
        let values=["https://example.test/API/\nAlice","https://example.test/api/\nAlice","https://example.test/api/\nalice","https://example.test/api/\náéç"];
        let targets:std::collections::HashSet<_>=values.map(|key|target(service,key)).into_iter().collect();assert_eq!(targets.len(),values.len());
        assert_ne!(target("ab","c"),target("a","bc"));assert_eq!(target(service,"user"),target(service,"user"));assert_eq!(target(service,&"x".repeat(10000)).len(),"br.com.papo.gtk.session.v2:".len()+64);
    }
}
