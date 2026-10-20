//! EN-US: One request budget for the session, including cloned clients and media jobs.
//! PT-BR: Um único orçamento de requisições por sessão, compartilhado por clientes clonados e tarefas de mídia.
use std::time::Duration;
use reqwest::{RequestBuilder, Response, StatusCode};
use tokio::{sync::Mutex, time::Instant};

const REQUEST_GAP: Duration = Duration::from_millis(125);
const REQUEST_BURST: u32 = 4;
const MAX_READ_RETRIES: u32 = 2;

pub(super) struct RequestScheduler {
    turn: Mutex<()>,
    state: Mutex<Admission>,
    gap:Duration,
}
struct Admission {
    next: Option<Instant>,
    cooldown: Option<Instant>,
    burst: u32,
}
impl Admission {
    fn deadline(&self, gap: Duration) -> Option<Instant> {
        let paced = self.next.and_then(|next| next.checked_sub(gap * (self.burst - 1)));
        paced.into_iter().chain(self.cooldown).max()
    }
}
impl Default for RequestScheduler {
    fn default() -> Self {
        Self { turn: Mutex::new(()), state: Mutex::new(Admission {
            next: None, cooldown: None, burst: REQUEST_BURST,
        }), gap: REQUEST_GAP }
    }
}

impl RequestScheduler {
    fn idle(&self)->bool{self.state.try_lock().is_ok_and(|state|state.deadline(self.gap).is_none_or(|at|at<=Instant::now()))}
    pub(super) fn remaining(&self)->Duration{self.state.try_lock().ok().and_then(|state|state.deadline(self.gap)).map_or(Duration::ZERO,|at|at.saturating_duration_since(Instant::now()))}
    pub(super) fn with_gap(gap:Duration)->Self{Self{turn:Mutex::new(()),state:Mutex::new(Admission{next:None,cooldown:None,burst:1}),gap}}
    async fn wait(&self) {
        let _timing=crate::performance::timer("request_queue_us");
        // FIFO admission prevents paginated background jobs from repeatedly
        // overtaking the selected chat. Cooldowns use a separate lock so an
        // in-flight response can extend the deadline while this task waits.
        let _turn = self.turn.lock().await;
        // EN-US: Reserve a slot only on admission so cancelled media jobs cannot delay login.
        // PT-BR: Reserva a vaga apenas na admissão para que tarefas de mídia canceladas não atrasem o login.
        loop {
            let mut state = self.state.lock().await;
            let now = Instant::now();
            let deadline = state.deadline(self.gap);
            if deadline.is_none_or(|deadline| deadline <= now) {
                // Up to four idle credits let a thumbnail batch start together;
                // refilling still admits only eight requests per second. Credits
                // belong to the whole session, never to individual media jobs.
                state.next = Some(state.next.unwrap_or(now).max(now) + self.gap);
                return;
            }
            drop(state);
            tokio::time::sleep_until(deadline.unwrap()).await;
        }
    }

    async fn cool_down(&self, delay: Duration) {
        let deadline = Instant::now() + delay;
        let mut state = self.state.lock().await;
        state.cooldown = Some(state.cooldown.map_or(deadline, |previous| previous.max(deadline)));
        // A server with a smaller or already exhausted budget must not see
        // another burst. Retry-After is separate from token refill so unused
        // credits can never bypass it, including on cloned clients.
        state.burst = 1;
    }
}

/// Bounded, process-wide public-resource budgets. Never evict a live request or
/// an origin still cooling down: doing so would bypass its Retry-After deadline.
pub(super) fn avatar_origin(origin:&str)->std::sync::Arc<RequestScheduler>{
    use std::sync::{Arc,LazyLock,Mutex};
    static ORIGINS:LazyLock<Mutex<std::collections::HashMap<String,Arc<RequestScheduler>>>>=LazyLock::new(Default::default);
    static OVERFLOW:LazyLock<Arc<RequestScheduler>>=LazyLock::new(||Arc::new(RequestScheduler::with_gap(Duration::from_millis(500))));
    let mut origins=ORIGINS.lock().unwrap_or_else(|e|e.into_inner());
    if let Some(budget)=origins.get(origin){return budget.clone();}
    if origins.len()>=64{let expired=origins.iter().find(|(_,v)|Arc::strong_count(v)==1&&v.idle()).map(|(k,_)|k.clone());if let Some(key)=expired{origins.remove(&key);}else{return OVERFLOW.clone();}}
    let budget=Arc::new(RequestScheduler::with_gap(Duration::from_millis(500)));origins.insert(origin.to_owned(),budget.clone());budget
}

fn retry_after(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    let value = headers.get(reqwest::header::RETRY_AFTER)?.to_str().ok()?.trim();
    if let Ok(seconds) = value.parse::<u64>() {
        // Avoid overflow in monotonic deadlines from an untrusted header.
        return Some(Duration::from_secs(seconds.min(86400)));
    }
    let date = chrono::DateTime::parse_from_rfc2822(value).ok()?;
    Some((date.with_timezone(&chrono::Utc) - chrono::Utc::now()).to_std().unwrap_or_default())
}

#[cfg(test)]
mod tests;

pub(super) trait PacedRequest {
    async fn send_paced(self, scheduler: &RequestScheduler) -> Result<Response, reqwest::Error>;
}

impl PacedRequest for RequestBuilder {
    async fn send_paced(self, scheduler: &RequestScheduler) -> Result<Response, reqwest::Error> {
        let template = self.try_clone();
        let metadata = template.as_ref().and_then(|b| b.try_clone()).and_then(|b| b.build().ok());
        let method = metadata.as_ref().map_or("UNKNOWN", |r| r.method().as_str());
        let read_only = metadata.as_ref().is_some_and(|r| r.method() == reqwest::Method::GET ||
                (r.method() == reqwest::Method::POST &&
                 ["/users/profile_batch", "/users/user_summary_batch"].iter().any(|p| r.url().path().ends_with(p))));
        let mut builder = self;
        for attempt in 0..=MAX_READ_RETRIES {
            scheduler.wait().await;
            let started = Instant::now();
            let result = builder.send().await.map_err(reqwest::Error::without_url);
            crate::performance::record("request_us",started.elapsed().as_micros() as u64);
            if result.as_ref().is_ok_and(|r|r.status()==StatusCode::TOO_MANY_REQUESTS){crate::performance::record("rate_limits",1);}
            // No URLs, headers, bodies or raw errors, even at debug/trace level.
            match &result {
                Ok(response) => tracing::debug!(method, attempt = attempt + 1, status = response.status().as_u16(), elapsed_ms = started.elapsed().as_millis(), "HTTP request completed"),
                Err(error) => tracing::debug!(method, attempt = attempt + 1, elapsed_ms = started.elapsed().as_millis(), reason = if error.is_timeout() { "timeout" } else if error.is_connect() { "connection" } else { "transport" }, "HTTP request failed"),
            }
            let response = result?;
            if response.status() != StatusCode::TOO_MANY_REQUESTS { return Ok(response); }
            let delay = retry_after(response.headers()).unwrap_or(Duration::from_secs(1 << attempt)).max(REQUEST_GAP);
            // EN-US: A rejected write shares the cooldown but is never replayed: it may have side effects.
            // PT-BR: Uma escrita rejeitada compartilha a pausa, mas não é repetida, pois pode ter efeitos colaterais.
            scheduler.cool_down(delay).await;
            if !read_only || attempt == MAX_READ_RETRIES || delay > Duration::from_secs(5) {
                return Ok(response);
            }
            let Some(retry) = template.as_ref().and_then(|b| b.try_clone()) else { return Ok(response); };
            drop(response);
            builder = retry;
        }
        unreachable!()
    }
}
