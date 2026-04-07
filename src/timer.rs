//! The purpose of this actor is to send [`Message::Update`] to [`gitlab_tokens_actor`](crate::state_actor::gitlab_tokens_actor)

use crate::config::Config;
use crate::state_actor::Message;
use core::time::Duration;
use tokio::{sync::mpsc, time};
use tracing::{error, info, instrument};

/// Sends [`Message::Update`] messages at a regular interval
#[instrument(skip_all)]
pub async fn timer_actor(config: Config, sender: mpsc::Sender<Message>) {
    let mut timer = time::interval(Duration::from_secs(
        u64::from(config.data_refresh_hours).saturating_mul(3600),
    ));

    info!(
        "refresh interval is {} hour{}",
        timer.period().as_secs().wrapping_div(3600),
        match config.data_refresh_hours {
            1 => "",
            _ => "s",
        }
    );

    loop {
        // The first call to timer.tick().await returns immediately
        timer.tick().await;
        match sender.send(Message::Update).await {
            Ok(()) => {}
            Err(err) => {
                error!("{err}");
                return;
            }
        }
    }
}
