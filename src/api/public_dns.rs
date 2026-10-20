//! EN-US: Validate the exact socket addresses passed to the HTTP connector, not just URLs.
//! PT-BR: Valida os endereços de conexão efetivamente entregues ao cliente HTTP, além da URL.
use std::net::{IpAddr,SocketAddr};
pub(super) fn public(ip:IpAddr)->bool{
    match ip{
        IpAddr::V4(ip)=>{let [a,b,c,_]=ip.octets();
            !(a==0||a==10||a==127||a>=224||(a==100&&(64..=127).contains(&b))||(a==169&&b==254)||(a==172&&(16..=31).contains(&b))
                ||(a==192&&(b==168||(b==0&&matches!(c,0|2))||(b==88&&c==99)))||(a==198&&(matches!(b,18|19)||(b==51&&c==100)))||(a==203&&b==0&&c==113))
        },
        IpAddr::V6(ip)=>{let s=ip.segments();(s[0]&0xe000)==0x2000&&s[0]!=0x2002&&!(s[0]==0x2001&&(s[1]<0x0200||s[1]==0x0db8))&&!(s[0]==0x3fff&&s[1]<0x1000)}
    }
}
// EN-US: Reject the entire DNS answer if any destination is non-public; a mixed answer must not bypass isolation.
// PT-BR: Rejeita toda a resposta DNS se algum destino não for público; respostas mistas não podem contornar o isolamento.
fn vetted(addresses:Vec<SocketAddr>)->std::io::Result<reqwest::dns::Addrs>{
    if addresses.is_empty()||addresses.iter().any(|a|!public(a.ip())){return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied,"External media requires public network addresses"));}
    Ok(Box::new(addresses.into_iter()))
}
pub(super) struct Resolver;
impl reqwest::dns::Resolve for Resolver{
    fn resolve(&self,name:reqwest::dns::Name)->reqwest::dns::Resolving{
        Box::pin(async move{let addresses=tokio::net::lookup_host((name.as_str(),443)).await?.take(65).collect::<Vec<_>>();
            if addresses.len()>64{return Err(std::io::Error::other("Too many DNS addresses").into());}
            Ok(vetted(addresses)?)
        })
    }
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn rejects_private_reserved_mapped_and_mixed_answers(){
        for ip in ["0.0.0.1","10.0.0.1","100.64.0.1","127.0.0.1","169.254.169.254","172.16.0.1","192.168.1.1","192.0.2.1","198.18.0.1","198.51.100.1","203.0.113.1","224.0.0.1","::1","::ffff:127.0.0.1","fc00::1","fe80::1","64:ff9b::7f00:1","2002:7f00:1::","2001:db8::1"]{assert!(!public(ip.parse().unwrap()),"{ip}");}
        for ip in ["1.1.1.1","8.8.8.8","2606:4700:4700::1111","2001:4860:4860::8888"]{assert!(public(ip.parse().unwrap()));}
        assert!(vetted(vec!["1.1.1.1:443".parse().unwrap(),"127.0.0.1:443".parse().unwrap()]).is_err());
    }
    #[tokio::test]async fn localhost_dns_cannot_reach_the_http_connector(){use reqwest::dns::Resolve;assert!(Resolver.resolve("localhost".parse().unwrap()).await.is_err());}
}
