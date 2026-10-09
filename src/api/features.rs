//! Authenticated media transfers and account contracts. Never retry message POSTs.
use super::*;
use futures_util::StreamExt;
use std::path::PathBuf;
use tokio::io:: {
    AsyncReadExt, AsyncWriteExt
};
pub const MAX_FILE: u64 = 100 << 20;
pub const MAX_BODY: u64 = 110 << 20;
#[derive(Debug, Clone)]
pub struct UploadFile {
    pub path: PathBuf, pub name: String, pub size: u64
}
impl UploadFile {
    pub async fn inspect(path: PathBuf) -> Result<Self> {
        let meta = tokio::fs::metadata(&path).await?;
        anyhow::ensure!(meta.is_file() && meta.len() <= MAX_FILE, "Escolha arquivos de até 100 MiB.");
        let name = path.file_name().context("Nome de arquivo inválido")?.to_string_lossy().into_owned();
        Ok(Self {
            path, name, size: meta.len()
        })
    }
}
pub fn validate_upload(content: Option<&str>, files: &[UploadFile]) -> Result<()> {
    validate_message(content,files,&[])
}
pub fn validate_message(content:Option<&str>,files:&[UploadFile],embeds:&[EmbedInput])->Result<()>{
    crate::models::validate_embeds(embeds)?;
    anyhow::ensure!(content.unwrap_or("").chars().count() <= 8192, "Use até 8192 caracteres.");
    anyhow::ensure!(files.len() <= 10, "Escolha até 10 arquivos.");
    anyhow::ensure!(files.iter().all(|f| f.size <= MAX_FILE), "Cada arquivo deve ter até 100 MiB.");
    // Reserve room for Unicode text, multipart boundaries and filenames.
    let bytes = files.iter().try_fold(0u64, |sum, f| sum.checked_add(f.size)).context("Arquivos muito grandes")?;
    anyhow::ensure!(bytes + content.unwrap_or("").len() as u64 + serde_json::to_vec(embeds)?.len() as u64 + 65536 <= MAX_BODY, "O envio deve ter até 110 MiB, incluindo o formulário.");
    anyhow::ensure!(!content.unwrap_or("").trim().is_empty() || !files.is_empty()||!embeds.is_empty(), "Escreva uma mensagem, escolha um arquivo ou adicione um embed.");
    Ok(())
}
/// Removes unfinished files on cancellation, errors, and session teardown.
#[derive(Debug)]
pub struct TemporaryFile(pub PathBuf);
impl Drop for TemporaryFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
/// Stage in the destination directory so a cancelled save never truncates an existing file.
pub async fn save_download(source: TemporaryFile, destination: PathBuf) -> Result<()> {
    let stage = destination.parent().context("Pasta de destino inválida")?.join(format!(".papo-download-{}", Uuid::new_v4()));
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)] options.mode(0o600);
    let mut output = options.open(&stage).await?;
    let stage = TemporaryFile(stage);
    let mut input = tokio::fs::File::open(&source.0).await?;
    let mut buffer = vec![0;
    65536];
    loop {
        let read = input.read(&mut buffer).await?;
        if read==0 {
            break;
        }
        output.write_all(&buffer[..read]).await?;
    }
    output.flush().await?;
    output.sync_all().await?;
    drop(output);
    tokio::fs::rename(&stage.0, destination).await?;
    Ok(())
}
impl ApiClient {
    pub async fn send_with_files(&self, request: &CreateMessageRequest, files: &[UploadFile], progress: impl Fn(u64) + Send + Sync + 'static) -> Result<Message> {
        validate_message(request.content.as_deref(), files,&request.embeds)?;
        let progress = Arc::new(progress);
        let count = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let mut form = reqwest::multipart::Form::new().text("channel_id", request.channel_id.to_string());
        if !request.embeds.is_empty(){form=form.text("embeds",serde_json::to_string(&request.embeds)?);}
        if let Some(text) = &request.content {
            form = form.text("content", text.clone());
        }
        if let Some(reply) = request.reply_to {
            form = form.text("reply_to", reply.to_string());
        }
        for selected in files {
            let file = tokio::fs::File::open(&selected.path).await?;
            anyhow::ensure!(file.metadata().await?.len() == selected.size, "O arquivo mudou; selecione-o novamente.");
            let count = count.clone();
            let progress = progress.clone();
            let stream = futures_util::stream::try_unfold((file, selected.size), move |(mut file, remaining)| {
                let count = count.clone();
                let progress = progress.clone();
                async move {
                    if remaining == 0 {
                        return Ok::<_, std::io::Error>(None);
                    }
                    let mut bytes = vec![0;
                    remaining.min(65536) as usize];
                    let read = file.read(&mut bytes).await?;
                    if read == 0 {
                        return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "O arquivo mudou durante o envio"));
                    }
                    bytes.truncate(read);
                    progress(count.fetch_add(read as u64, std::sync::atomic::Ordering::Relaxed) + read as u64);
                    Ok(Some((bytes, (file, remaining - read as u64))))
                }
            });
            form = form.part("attachments", reqwest::multipart::Part::stream_with_length(reqwest::Body::wrap_stream(stream), selected.size).file_name(selected.name.clone()));
        }
        Self::decode(self.inner.post(self.url("/messages")).timeout(Duration::from_secs(600))
        .multipart(form).send_paced(&self.requests).await?, "enviar arquivos").await
    }
    pub async fn media_bytes(&self, path: &str, maximum: usize) -> Result<Vec<u8>> {
        let response = Self::check_response(self.inner.get(self.url(path)).send_paced(&self.requests).await?, "carregar mídia").await?;
        anyhow::ensure!(response.content_length().unwrap_or(0) <= maximum as u64, "Imagem muito grande.");
        let mut stream = response.bytes_stream();
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            anyhow::ensure!(bytes.len() + chunk.len() <= maximum, "Imagem muito grande.");
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
    pub async fn download_temporary(&self, endpoint: &str, maximum: u64) -> Result<TemporaryFile> {
        let response = Self::check_response(self.inner.get(self.url(endpoint)).timeout(Duration::from_secs(600)).send_paced(&self.requests).await?, "baixar mídia").await?;
        anyhow::ensure!(response.content_length().unwrap_or(0) <= maximum, "Arquivo muito grande.");
        let path = std::env::temp_dir().join(format!("papo-media-{}", Uuid::new_v4()));
        let mut options = tokio::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)] options.mode(0o600);
        let mut file = options.open(&path).await?;
        let temporary = TemporaryFile(path);
        let mut stream = response.bytes_stream();
        let mut total = 0;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            total += chunk.len() as u64;
            anyhow::ensure!(total <= maximum, "Arquivo muito grande.");
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        Ok(temporary)
    }
    pub async fn update_profile(&self, id: Uuid, request: &UpdateUserRequest) -> Result<()> {
        anyhow::ensure!(request.nickname.chars().count() <= 32 && request.status.chars().count() <= 64
        && request.description.chars().count() <= 512 && request.typing.as_deref().unwrap_or("").chars().count() <= 64, "Perfil excede os limites de caracteres.");
        Self::check_response(self.inner.put(self.url(&format!("/users/{id}"))).json(request).send_paced(&self.requests).await?, "salvar perfil").await?;
        Ok(())
    }
    pub async fn update_status(&self, id: Uuid, status: Option<UserStatus>) -> Result<()> {
        Self::check_response(self.inner.put(self.url(&format!("/users/{id}/status"))).json(&UpdateStatusRequest {
            status
        }).send_paced(&self.requests).await?, "salvar presença").await?;
        Ok(())
    }
    pub async fn update_image(&self, id: Uuid, banner: bool, bytes: &[u8]) -> Result<()> {
        use base64::Engine;
        let format = if bytes.is_empty() {
            ""
        } else {
            validate_profile_image(bytes, banner)?
        };
        let value = base64::engine::general_purpose::STANDARD.encode(bytes);
        let (endpoint, body) = if banner {
            ("banner", serde_json::json!( {
                "banner": value, "banner_format": format
            }))
        }
        else {
            ("avatar", serde_json::json!( {
                "avatar": value, "avatar_format": format
            }))
        };
        Self::check_response(self.inner.put(self.url(&format!("/users/{id}/{endpoint}"))).json(&body).send_paced(&self.requests).await?, "salvar imagem").await?;
        Ok(())
    }
    pub async fn save_settings(&self, config: &UserConfig) -> Result<UserSettings> {
        Self::decode(self.inner.put(self.url("/users/settings")).json(&serde_json::json!( {
            "config": config.complete()
        })).send_paced(&self.requests).await?, "salvar preferências").await
    }
    pub async fn channel_notifications(&self, channel: Uuid, user: Uuid, notification_settings: NotificationSettings) -> Result<()> {
        Self::check_response(self.inner.post(self.url(&format!("/channels/{channel}/user/{user}/settings")))
        .json(&UpdateChannelUserSettingRequest {
            notification_settings
        }).send_paced(&self.requests).await?, "preferências do canal").await?;
        Ok(())
    }
}
pub fn validate_profile_image(bytes: &[u8], banner: bool) -> Result<&'static str> {
    anyhow::ensure!(bytes.len() <= 2 << 20, "A imagem deve ter até 2 MiB.");
    let format = image::guess_format(bytes)?;
    let name = match format {
        image::ImageFormat::Png => "PNG", image::ImageFormat::Jpeg => "JPEG", image::ImageFormat::Gif => "GIF", image::ImageFormat::WebP => "WEBP", _ => anyhow::bail!("Use PNG, JPEG, GIF ou WebP.")
    };
    let (w, h) = image::ImageReader::with_format(std::io::Cursor::new(bytes), format).into_dimensions()?;
    let max = if banner {
        2048
    } else {
        512
    };
    anyhow::ensure!(w > 0 && h > 0 && w <= max && h <= max, "A imagem deve ter dimensões de até {max} pixels.");
    image::load_from_memory_with_format(bytes, format)?;
    Ok(name)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upload_limits_count_unicode_and_all_multipart_files() {
        let file = UploadFile {
            path: "unused".into(), name: "test".into(), size: MAX_FILE
        };
        assert!(validate_upload(Some(&"🌎".repeat(8192)), &[]).is_ok());
        assert!(validate_upload(Some(&"🌎".repeat(8193)), &[]).is_err());
        assert!(validate_upload(None, std::slice::from_ref(&file)).is_ok());
        assert!(validate_upload(None, &[file.clone(), file.clone()]).is_err());
        assert!(validate_upload(None, &[]).is_err());
        assert!(validate_upload(None, &vec![UploadFile {
            size: 1, ..file.clone()
        };
        11]).is_err());
        assert!(validate_upload(None, &[UploadFile {
            size: MAX_FILE+1, ..file
        }]).is_err());
    }
    #[test]
    fn complete_settings_preserve_false_values_and_fill_legacy_fields() {
        let c: UserConfig = serde_json::from_value(serde_json::json!( {
            "display": {
                "showAvatars": false
            }, "notifications": {
                "sound": false
            }
        })).unwrap();
        let value = serde_json::to_value(c.complete()).unwrap();
        assert_eq!(value["theme"], "system");
        assert_eq!(value["display"]["showAvatars"], false);
        assert_eq!(value["display"]["fontSize"], "medium");
        assert_eq!(value["notifications"]["sound"], false);
        assert_eq!(value["notifications"]["messagePreview"], true);
    }
    #[test]
    fn profile_images_validate_size_format_and_distinct_banner_dimensions() {
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(513, 1).write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        assert!(validate_profile_image(bytes.get_ref(), false).is_err());
        assert_eq!(validate_profile_image(bytes.get_ref(), true).unwrap(), "PNG");
        assert!(validate_profile_image(&vec![0;
        (2<<20)+1], true).is_err());
        assert!(validate_profile_image(b"not an image", false).is_err());
    }
}
