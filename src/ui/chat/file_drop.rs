//! Native file drops stage attachments in the current conversation without sending them.
use super::*;
use crate::api::features::UploadFile;

impl ChatModel {
    fn accepts_file_drop(&self) -> bool {
        !self.actions.pinned_view && self.active_channel.is_some()
            && self.access.send && self.access.attachments
            && !self.persistence.loading && self.transfers.clipboard_request.is_none()
    }

    pub(super) fn install_file_drop(&mut self, root: &gtk::Box, sender: &ComponentSender<Self>) {
        if self.actions.pinned_view { return; }
        let target = gtk::DropTarget::new(gtk::gdk::FileList::static_type(), gtk::gdk::DragAction::empty());
        target.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = root.downgrade();
        target.connect_enter(move |target, _, _| {
            if target.actions().is_empty() { return gtk::gdk::DragAction::empty(); }
            if let Some(root) = weak.upgrade() { root.add_css_class("papo-file-drop"); }
            gtk::gdk::DragAction::COPY
        });
        let weak = root.downgrade();
        target.connect_leave(move |_| {
            if let Some(root) = weak.upgrade() { root.remove_css_class("papo-file-drop"); }
        });
        let input = sender.input_sender().clone();
        let weak = root.downgrade();
        target.connect_drop(move |target, value, _, _| {
            if let Some(root) = weak.upgrade() { root.remove_css_class("papo-file-drop"); }
            if target.actions().is_empty() { return false; }
            let Ok(files) = value.get::<gtk::gdk::FileList>() else { return false; };
            input.send(ChatMsg::Transfer(TransferMsg::DropFiles(files.files()))).is_ok()
        });
        root.add_controller(target.clone());
        self.transfers.file_drop = Some(target);
        self.update_file_drop(root);
    }

    pub(super) fn update_file_drop(&self, root: &gtk::Box) {
        if let Some(target) = &self.transfers.file_drop {
            let allowed = self.accepts_file_drop();
            target.set_actions(if allowed { gtk::gdk::DragAction::COPY } else { gtk::gdk::DragAction::empty() });
            if !allowed { root.remove_css_class("papo-file-drop"); }
        }
    }

    pub(super) fn drop_files(&mut self, files: Vec<gtk::gio::File>, sender: &ComponentSender<Self>) {
        if !self.accepts_file_drop() || files.is_empty() { return; }
        if files.len() + self.draft.files.len() > 10 {
            self.draft.error = Some(crate::i18n::tr("Escolha até 10 arquivos.").into());
            return;
        }
        let Some(paths) = files.iter().map(gtk::gio::File::path).collect::<Option<Vec<_>>>() else {
            self.draft.error = Some(crate::i18n::tr("Escolha arquivos locais para anexar.").into());
            return;
        };
        let channel = self.active_channel.as_ref().unwrap().id();
        let epoch = self.transfers.epoch;
        let request = Uuid::new_v4();
        self.transfers.clipboard_request = Some(request);
        let input = sender.input_sender().clone();
        self.transfers.jobs.push(tokio::spawn(async move {
            let result = async {
                let mut files = Vec::new();
                for path in paths { files.push(UploadFile::inspect(path).await?); }
                Ok(files)
            }.await;
            let _ = input.send(ChatMsg::Transfer(TransferMsg::PasteReady { epoch, channel, request, result }));
        }));
    }
}
