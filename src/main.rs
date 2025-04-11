//! Export the number of days before GitLab tokens expire as Prometheus metrics.

mod gitlab;
mod prometheus_metrics;
mod state_actor;
mod timer;

use axum::{Router, extract::State, http::StatusCode, routing::get};
use core::future::IntoFuture as _; // To be able to use into_future()
use std::io::{Error, ErrorKind};
use tokio::{
    net::TcpListener,
    select,
    signal::unix::{SignalKind, signal},
    sync::{mpsc, oneshot},
};
use tracing::{info, instrument};
use tracing_subscriber::EnvFilter;

use crate::state_actor::{ActorState, Message, gitlab_tokens_actor};
use crate::timer::timer_actor;

/// Handles `/metrics` requests
async fn get_gitlab_tokens_handler(
    State(sender): State<mpsc::Sender<Message>>,
) -> (StatusCode, String) {
    // We are going to send a message to our actor and wait for an answer
    // But first, we create a oneshot channel to get the actor's response
    let (send, recv) = oneshot::channel();
    let msg = Message::Get { respond_to: send };

    // Ignore send errors. If this send fails, so does the
    // recv.await below. There's no reason to check for the
    // same failure twice.
    #[expect(clippy::let_underscore_must_use, reason = "Ignore send errors")]
    #[expect(clippy::let_underscore_untyped, reason = "Ignore send errors type")]
    let _ = sender.send(msg).await;

    match recv.await {
        Ok(res) => match res {
            ActorState::Loading | ActorState::NoToken => (StatusCode::NO_CONTENT, String::new()),
            ActorState::Loaded(state) => (StatusCode::OK, state),
            ActorState::Error(err) => (StatusCode::INTERNAL_SERVER_ERROR, err),
        },
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

/// Static response for requests on `/`
async fn root_handler() -> &'static str {
    "I'm Alive :D"
}

#[expect(
    clippy::integer_division_remainder_used,
    reason = "Because clippy is not happy with the tokio::select macro #1"
)]
#[tokio::main(flavor = "current_thread")]
#[instrument]
async fn main() -> Result<(), Error> {
    // Configure tracing_subscriber with a custom formatter
    #[expect(clippy::absolute_paths, reason = "Only call to this function")]
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("INFO")),
        )
        .event_format(
            tracing_subscriber::fmt::format()
                .with_target(false)
                .compact(),
        )
        .init();

    // An infinite stream of 'SIGTERM' signals.
    let mut sigterm_stream = signal(SignalKind::terminate())?;

    // Create a channel and then our main actor, gitlab_tokens_actor()
    let (sender, receiver) = mpsc::channel(8);
    let gitlab_tokens_actor_handle = tokio::spawn(gitlab_tokens_actor(receiver, sender.clone()));

    // Create the timer actor
    let timer_actor_handle = tokio::spawn(timer_actor(sender.clone()));

    let app = Router::new()
        .route("/", get(root_handler))
        .route("/metrics", get(get_gitlab_tokens_handler))
        .with_state(sender);

    let listener = TcpListener::bind("0.0.0.0:3000").await?;

    let local_addr = listener.local_addr()?;

    info!("listening on {local_addr}");

    // We are waiting for one of the following :
    // - a SIGTERM signal
    // - the state actor to finish/panic
    // - the timer actor to finish/panic
    // - the axum server to finish
    select! {
        _ = sigterm_stream.recv() => {
            return Err(Error::new(ErrorKind::Other, "Received a SIGTERM signal!"));
        },
        _ = gitlab_tokens_actor_handle => {
            return Err(Error::new(ErrorKind::Other, "The state actor died!"));
        },
        _ = timer_actor_handle => {
            return Err(Error::new(ErrorKind::Other, "The timer actor died!"));
        },
        _ = axum::serve(listener, app).into_future() => {
            return Err(Error::new(ErrorKind::Other, "The server died!"));
        }
    }
}
