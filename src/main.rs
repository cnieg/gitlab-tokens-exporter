//! Export the number of days before GitLab tokens expire as Prometheus metrics.

mod error;
mod gitlab;
mod prometheus_metrics;
mod state_actor;
mod timer;

use axum::{Router, extract::State, http::StatusCode, routing::get};
use std::io::Error;
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

/// This function waits for a 'SIGTERM' signal
#[expect(clippy::expect_used, reason = "Exit if we can't create a listener")]
async fn shutdown_signal() {
    let mut sigterm_stream =
        signal(SignalKind::terminate()).expect("Failed to create a SIGTERM listener");

    sigterm_stream.recv().await;
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
    // - the state actor to finish/panic
    // - the timer actor to finish/panic
    // - the axum server to finish/be interrupted by a SIGTERM
    select! {
        _ = gitlab_tokens_actor_handle => {
            return Err(Error::other("The state actor died!"));
        },
        _ = timer_actor_handle => {
            return Err(Error::other("The timer actor died!"));
        },
        _ = axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()) => {
            return Err(Error::other("The server received a SIGTERM or died!"));
        }
    }
}
