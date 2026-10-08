//! One request budget for the session, including cloned clients and media jobs.
use std::time::Duration;
use reqwest::{RequestBuilder, Response, StatusCode};
use tokio::{sync::Mutex, time::Instant};

const REQUEST_GAP: Duration = Duration::from_millis(125);
const MAX_READ_RETRIES: u32 = 2;

#[derive(Default)]
pub(super) struct RequestScheduler {
    turn: Mutex<()>,
    next: Mutex<Option<Instant>>,
}

impl RequestScheduler {
    async fn wait(&self) {
        // FIFO admission prevents paginated background jobs from repeatedly
        // overtaking the selected chat. Cooldowns use a separate lock so an
        // in-flight response can extend the deadline while this task waits.
        let _turn = self.turn.lock().await;
        // Do not reserve future slots: cancelled media jobs must not delay login.
        loop {
            let mut next = self.next.lock().await;
            let now = Instant::now();
            if next.map_or(true, |deadline| deadline <= now) {
                *next = Some(now + REQUEST_GAP);
                return;
            }
            let deadline = next.unwrap();
            drop(next);
            tokio::time::sleep_until(deadline).await;
        }
    }

    async fn cool_down(&self, delay: Duration) {
        let deadline = Instant::now() + delay;
        let mut next = self.next.lock().await;
        *next = Some(next.map_or(deadline, |previous| previous.max(deadline)));
    }
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
        let read_only = template.as_ref().and_then(|b| b.try_clone()).and_then(|b| b.build().ok())
            .is_some_and(|r| r.method() == reqwest::Method::GET ||
                (r.method() == reqwest::Method::POST &&
                 ["/users/profile_batch", "/users/user_summary_batch"].iter().any(|p| r.url().path().ends_with(p))));
        let mut builder = self;
        for attempt in 0..=MAX_READ_RETRIES {
            scheduler.wait().await;
            let response = builder.send().await?;
            if response.status() != StatusCode::TOO_MANY_REQUESTS { return Ok(response); }
            let delay = retry_after(response.headers()).unwrap_or(Duration::from_secs(1 << attempt)).max(REQUEST_GAP);
            // Even a rejected mutation cools down all other tasks; never replay it.
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
