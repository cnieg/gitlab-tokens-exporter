//! The purpose of this actor is to parse the `DATA_REFRESH_HOURS` environment
//! variable and to send [`StateActorMessage::Update`] to `gitlab_tokens_actor`

use crate::state_actor::Message;
use core::time::Duration;
use std::env;
use tokio::{sync::mpsc, time};
use tracing::{error, info, instrument};

/// Default value for `data_refresh_hours`
const DATA_REFRESH_HOURS_DEFAULT: u8 = 6;

/// Sends `Message::Update` messages at a regular interval
#[instrument(skip_all, target = "timer")]
pub async fn timer_actor(sender: mpsc::Sender<Message>) {
    let data_refresh_hours =
        env::var("DATA_REFRESH_HOURS").map_or(DATA_REFRESH_HOURS_DEFAULT, |env_value| {
            env_value
                .parse()
                .map_or(DATA_REFRESH_HOURS_DEFAULT, |env_value_u8| {
                    if env_value_u8 > 0 && env_value_u8 <= 24 {
                        env_value_u8
                    } else {
                        DATA_REFRESH_HOURS_DEFAULT
                    }
                })
        });

    let mut timer = time::interval(Duration::from_secs(
        u64::from(data_refresh_hours).saturating_mul(3600),
    ));

    info!(
        "refresh interval is {} hours",
        timer.period().as_secs().wrapping_div(3600)
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
