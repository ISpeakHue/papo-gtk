//! EN-US: Account-owned transport and recovery state, independent of GTK windows.
//! PT-BR: Transporte e recuperação pertencem à conta e independem das janelas GTK.
use std::time::{Duration, Instant};
use tokio::{sync::mpsc, task::JoinHandle};
use crate::{account::AccountId, api::ApiClient, models::UserConfig, ws};

pub struct Connection {
    pub account: AccountId,
    pub api: ApiClient,
    pub config: UserConfig,
    pub ws_tx: mpsc::Sender<ws::WsCommand>,
    pub state: State,
    pub refresh_task: Option<JoinHandle<()>>,
    pub ws_listener: Option<JoinHandle<()>>,
    pub cleanup_task: Option<JoinHandle<()>>,
    pub startup_task: Option<JoinHandle<()>>,
}

#[derive(Default)]
pub struct State {
    pub last_activity: Option<Instant>,
    pub last_access_refresh: Option<Instant>,
    pub socket_ready: bool,
    pub had_connection: bool,
    pub reconnect_pending: bool,
    pub history_recovering: bool,
    pub access_valid: bool,
    pub published_status: Option<String>,
    pub startup_started: bool,
    pub startup_on_socket: bool,
    pub background_started: bool,
    pub poll_jitter: u64,
}

impl Connection {
    pub fn new(api: ApiClient, user: uuid::Uuid, config: UserConfig) -> (Self, mpsc::Receiver<ws::WsEvent>) {
        let account = AccountId::new(&api, user);
        debug_assert_eq!(account.server(),api.base_url());
        debug_assert_eq!(account.user(),user);
        let (receiver, ws_tx) = ws::spawn(api.clone());
        (Self { account, api, config, ws_tx,
            state: State { history_recovering: true, poll_jitter: (uuid::Uuid::new_v4().as_u128() % 6) as u64, ..Default::default() },
            refresh_task: None, ws_listener: None, cleanup_task: None, startup_task: None }, receiver)
    }
}

impl State {
    pub fn status(&self, delay: Duration, access_pending: bool) -> Option<String> {
        if delay > Duration::from_secs(1) { Some(crate::i18n::fmt("O servidor pediu uma pausa. Nova tentativa em {} s…", &[("0", format!("{}", delay.as_secs() + 1))])) }
        else if !self.socket_ready { Some(if self.had_connection { crate::i18n::tr("Reconectando… O histórico em cache continua disponível.") } else { crate::i18n::tr("Conectando ao servidor…") }.into()) }
        else if !self.access_valid && !access_pending { Some(crate::i18n::tr("Sem atualização de permissões. Tentaremos novamente automaticamente.").into()) }
        else if self.history_recovering || !self.access_valid && access_pending { Some(crate::i18n::tr("Atualizando mensagens e permissões…").into()) }
        else { None }
    }

    pub fn refresh_interval(&self, active: bool) -> Duration {
        Duration::from_secs(if self.socket_ready && !active { 90 + self.poll_jitter } else { 30 + self.poll_jitter })
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        // EN-US: Dropping the listener closes the event receiver and stops its WebSocket loop.
        // PT-BR: Ao encerrar o listener, o receptor de eventos fecha e interrompe seu loop WebSocket.
        for job in [&mut self.refresh_task, &mut self.ws_listener, &mut self.cleanup_task, &mut self.startup_task] {
            if let Some(job) = job.take() { job.abort(); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_status_and_polling_are_independent_per_connection() {
        let mut a = State { poll_jitter: 2, ..Default::default() };
        let b = State::default();
        assert!(a.status(Duration::ZERO, false).unwrap().starts_with("Conectando"));
        a.socket_ready = true; a.had_connection = true; a.history_recovering = true;
        assert!(a.status(Duration::ZERO, true).unwrap().starts_with("Atualizando"));
        assert!(a.status(Duration::ZERO, false).unwrap().starts_with("Sem atualização"));
        a.access_valid = true; a.history_recovering = false;
        assert_eq!(a.status(Duration::ZERO, false), None);
        assert!(a.status(Duration::from_secs(3), false).unwrap().contains("4 s"));
        assert_eq!(a.refresh_interval(false), Duration::from_secs(92));
        assert_eq!(a.refresh_interval(true), Duration::from_secs(32));
        assert!(!b.socket_ready && !b.access_valid);
    }

    #[tokio::test]
    async fn dropping_one_account_cancels_its_jobs_without_stopping_another() {
        let api = ApiClient::new("http://127.0.0.1:9").unwrap();
        let (mut a, rx_a) = Connection::new(api.clone(), uuid::Uuid::new_v4(), Default::default());
        let second_api=ApiClient::new(api.base_url()).unwrap();
        let (mut b, rx_b) = Connection::new(second_api, uuid::Uuid::new_v4(), Default::default());
        assert_ne!(a.account,b.account);
        a.config.theme=Some(crate::models::Theme::Dark);
        assert_ne!(a.config.theme,b.config.theme);
        let task = || tokio::spawn(std::future::pending::<()>());
        a.cleanup_task = Some(task()); b.cleanup_task = Some(task());
        let stopped = a.cleanup_task.as_ref().unwrap().abort_handle();
        let live = b.cleanup_task.as_ref().unwrap().abort_handle();
        drop(a); drop(rx_a);
        tokio::time::timeout(Duration::from_secs(1),async{while !stopped.is_finished(){tokio::task::yield_now().await;}}).await.unwrap();
        assert!(stopped.is_finished()); assert!(!live.is_finished());
        drop(b); drop(rx_b);
    }
}
