//! Bounded first-frame/thumbnail residency for the latest opening batch.
use super::*;

const BYTE_LIMIT: u64 = 192 << 20;
const ENTRY_LIMIT: usize = 256;
const CHANNEL_LIMIT: usize = 8;
struct Entry {
    texture: gtk::gdk::Texture,
    messages: HashSet<Uuid>,
    bytes: u64,
}
#[derive(Default)]
pub(in crate::ui::chat) struct Cache {
    entries: HashMap<(Uuid, Uuid), Entry>,
    recent: HashMap<Uuid,HashSet<Uuid>>,
    order: VecDeque<(Uuid, Uuid)>,
    channels: VecDeque<Uuid>,
    bytes: u64,
}
impl Cache {
    pub fn contains(&self, channel: Uuid, key: Uuid) -> bool { self.entries.contains_key(&(channel,key)) }
    pub fn select(&mut self, channel: Uuid, keys: &HashMap<Uuid, HashSet<Uuid>>) {
        self.channels.retain(|id| *id != channel);
        self.channels.push_back(channel);
        while self.channels.len() > CHANNEL_LIMIT {
            if let Some(old) = self.channels.pop_front() { self.invalidate_channel(old); }
        }
        self.recent.insert(channel,keys.keys().copied().collect());
        for (key,messages) in keys {
            if let Some(entry) = self.entries.get_mut(&(channel,*key)) { entry.messages = messages.clone(); }
        }
    }
    pub fn get(&mut self, channel: Uuid, key: Uuid) -> Option<gtk::gdk::Texture> {
        let texture = self.entries.get(&(channel,key))?.texture.clone();
        self.order.retain(|id| *id != (channel,key));
        self.order.push_back((channel,key));
        crate::performance::record("resident_preview_hits",1);
        Some(texture)
    }
    pub fn insert(&mut self, channel: Uuid, key: Uuid, messages: &HashSet<Uuid>, texture: &gtk::gdk::Texture) {
        self.remove((channel,key));
        let bytes = texture.width() as u64 * texture.height() as u64 * 4;
        if bytes > BYTE_LIMIT { return; }
        self.bytes += bytes;
        self.entries.insert((channel,key),Entry { texture:texture.clone(),messages:messages.clone(),bytes });
        self.order.push_back((channel,key));
        while self.bytes > BYTE_LIMIT || self.entries.len() > ENTRY_LIMIT {
            let Some(key) = self.order.iter().find(|(channel,key)|!self.recent.get(channel).is_some_and(|keys|keys.contains(key))).or_else(||self.order.front()).copied() else { break; };
            self.remove(key);
        }
        crate::performance::record("resident_preview_bytes",self.bytes);
    }
    fn remove(&mut self, key: (Uuid,Uuid)) {
        if let Some(entry) = self.entries.remove(&key) { self.bytes -= entry.bytes; }
        self.order.retain(|id| *id != key);
    }
    pub fn invalidate(&mut self, key: Uuid) {
        let keys: Vec<_> = self.entries.keys().copied().filter(|(_,id)| *id == key).collect();
        for key in keys { self.remove(key); }
    }
    pub fn invalidate_channel(&mut self, channel: Uuid) {
        let keys: Vec<_> = self.entries.keys().copied().filter(|(id,_)| *id == channel).collect();
        for key in keys { self.remove(key); }
        self.channels.retain(|id| *id != channel);
        self.recent.remove(&channel);
    }
    pub fn retain_channels(&mut self, allowed: &[Uuid]) {
        let removed: Vec<_> = self.channels.iter().copied().filter(|id| !allowed.contains(id)).collect();
        for id in removed { self.invalidate_channel(id); }
    }
    pub fn apply(&mut self, change: &Change) {
        match change {
            Change::Delete(message) => {
                let mut removed = vec![];
                for (key,entry) in &mut self.entries {
                    entry.messages.remove(message);
                    if entry.messages.is_empty() { removed.push(*key); }
                }
                for key in removed { self.remove(key); }
            }
            Change::Preview(_,embed) => self.invalidate(embed.id),
            Change::RemovePreview(_,key) => self.invalidate(*key),
            Change::Embeds(message,_) => {
                let keys:Vec<_> = self.entries.iter().filter(|(_,entry)|entry.messages.contains(message)).map(|(key,_)|*key).collect();
                for key in keys { self.remove(key); }
            }
            Change::Moderation(_,key,status) if !matches!(status.as_str(),"clean"|"failed"|"") => self.invalidate(*key),
            _ => {},
        }
    }
    pub fn clear(&mut self) { *self = Self::default(); }
}

impl Transfers {
    pub(super) fn select_recent(&mut self, channel: Uuid, messages: &[Message], range: std::ops::Range<usize>) {
        self.recent_channel = Some(channel);
        self.recent_keys.clear();self.preview_owners.clear();
        self.visible_keys.clear();
        let mut animated = HashSet::new();
        let recent_start=messages.len().saturating_sub(crate::models::INITIAL_MESSAGE_LIMIT);
        let start=range.start.min(recent_start);let end=range.end.min(recent_start);
        let records=messages[start..end].iter().enumerate().map(|(i,m)|(start+i,m))
            .chain(messages[recent_start..].iter().enumerate().map(|(i,m)|(recent_start+i,m)));
        for (index,message) in records {
            let recent = index >= messages.len().saturating_sub(crate::models::INITIAL_MESSAGE_LIMIT);
            let visible = self.scope.contains(&message.id);
            if !recent && !visible { continue; }
            let attachments = message.attachments.iter().flatten().filter(|a|self.visible(a))
                .map(|a|(a.id,a.mime_type == "image/gif"));
            let embeds = message.embeds.iter().flatten().map(|e|(e.id,e.animated_link()));
            let media:Vec<_> = attachments.chain(embeds).collect();
            for (key,gif) in media {
                self.preview_owners.entry(key).or_default().insert(message.id);
                if recent { self.recent_keys.entry(key).or_default().insert(message.id); }
                if visible { self.visible_keys.insert(key); }
                if gif { animated.insert(key); }
            }
        }
        self.resident.select(channel,&self.recent_keys);
        self.encoded.set_recent(channel,self.recent_keys.keys().copied().collect());
        for (key,messages) in &self.recent_keys {
            if !self.resident.contains(channel,*key) {
                if let Some(texture)=self.textures.get(key) { self.resident.insert(channel,*key,messages,texture); }
            }
        }
        // Only mount the visible portion now. Other recent previews remain in
        // the resident cache and need neither another download nor a decode.
        let restore: Vec<_> = self.visible_keys.iter().copied().filter(|key|!self.textures.contains_key(key)).collect();
        for key in restore {
            if let Some(texture) = self.resident.get(channel,key) {
                self.dirty.insert(key);
                self.revision = self.revision.wrapping_add(1);
                self.textures.insert(key,texture);
                self.order.retain(|id|*id != key); self.order.push_back(key);
                if animated.contains(&key) { self.static_previews.insert(key); }
            }
        }
        self.prune_textures();
        self.static_previews.retain(|key|self.textures.contains_key(key));
    }
    pub(super) fn remember_resident(&mut self, key: Uuid, texture: &gtk::gdk::Texture) {
        if let Some(channel) = self.recent_channel {
            if let Some(messages)=self.preview_owners.get(&key){self.resident.insert(channel,key,messages,texture);}
        }
    }
}
