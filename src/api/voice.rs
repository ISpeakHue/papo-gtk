use super::*;
impl ApiClient {
    pub async fn voice_ice_servers(&self)->Result<IceConfig>{
        let config:IceConfig=Self::decode(self.inner.get(self.url("/voice/ice-servers")).send().await?,"consultar servidores ICE").await?;
        for server in &config.ice_servers{for url in &server.urls{anyhow::ensure!(url.starts_with("stun:")||url.starts_with("turn:")||url.starts_with("turns:"),"Configuração ICE contém um protocolo incompatível.");}}
        Ok(config)
    }
}
