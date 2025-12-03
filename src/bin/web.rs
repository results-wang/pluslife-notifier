use std::{collections::BTreeMap, net::Ipv4Addr};

use askama::Template;
use axum::{
    Form, Json, Router,
    extract::Path,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
};
use axum_embed::ServeEmbed;
use dotenv::dotenv;
use email_address::EmailAddress;
use jiff::Timestamp;
use pluslife_notifier::{
    messages::Message,
    notifier::{notify, notify_error},
    sessions::{ServerState, Session},
    state::State,
};
use rust_embed::RustEmbed;
use serde::Deserialize;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info, trace};
use tracing_subscriber::{Layer, layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

#[derive(RustEmbed, Clone)]
#[folder = "src/static/"]
struct StaticAssets;

#[tokio::main]
async fn main() {
    if let Err(err) = dotenv()
        && !err.not_found()
    {
        panic!("Error loading .env file: {}", err);
    }

    let stderr_log_level = tracing_subscriber::filter::LevelFilter::INFO;
    let stderr_layer = tracing_subscriber::fmt::layer()
        .pretty()
        .with_writer(std::io::stderr);

    tracing_subscriber::registry()
        .with(stderr_layer.with_filter(stderr_log_level))
        .try_init()
        .expect("Failed to configure logging");

    let port = std::env::var("PORT").expect("Expected PORT to be set");
    let port = port.parse::<u16>().expect("Expected PORT to be a number");

    let static_assets = ServeEmbed::<StaticAssets>::new();

    let cors = CorsLayer::new()
        .allow_methods(Any)
        .allow_headers(Any)
        .allow_origin(Any);

    let server_state = ServerState::try_from_env().expect("Failed to read config from env");

    info!(base_url = server_state.base_url, port, "Starting server");

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/session/create", post(create_session))
        .route("/session/{id}/data", post(receive_data))
        .route("/session/{id}/data", get(get_data_dummy))
        .route("/session/{id}/graph", get(graph_data))
        .route("/dump", post(print_json_data))
        .route("/sessions/count", get(count_sessions))
        .layer(cors)
        .with_state(server_state)
        .fallback_service(static_assets);

    let listener = tokio::net::TcpListener::bind((Ipv4Addr::from_octets([0, 0, 0, 0]), port))
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(Deserialize)]
struct CreateSessionRequest {
    email: EmailAddress,
}

#[derive(Template)]
#[template(path = "session-created.html")]
struct CreateSessionResponse {
    pub base_url: String,
    pub id: Uuid,
    pub email_to_notify: EmailAddress,
}

async fn create_session(
    axum::extract::State(server_state): axum::extract::State<ServerState>,
    Form(params): Form<CreateSessionRequest>,
) -> Html<String> {
    let id = server_state.create_session(params.email.clone());
    info!(%id, email = %params.email, "Created session");
    Html(
        CreateSessionResponse {
            id,
            base_url: server_state.base_url.clone(),
            email_to_notify: params.email,
        }
        .render()
        .unwrap(),
    )
}

async fn receive_data(
    Path(id): Path<Uuid>,
    axum::extract::State(server_state): axum::extract::State<ServerState>,
    Json(message): Json<Message>,
) -> impl IntoResponse + Send {
    let mut sessions = server_state.sessions.lock().unwrap();
    if let Some(session) = sessions.remove(&id) {
        let Session {
            state,
            created,
            email_to_notify,
            id,
        } = session;
        let event = message.event;
        let state = state.update(message);
        match state {
            Ok(State::CompletedTest(completed_test)) => {
                info!(%id, "Received results");
                tokio::spawn(async move {
                    let notify_result = notify(
                        &server_state.sender_email,
                        &server_state.mailgun_domain,
                        &server_state.mailgun_api_key,
                        completed_test,
                        email_to_notify.clone(),
                    )
                    .await;
                    if let Err(err) = notify_result {
                        error!(?err, "Error notifying of result");
                        let _ = notify_error(
                            &server_state.sender_email,
                            &server_state.mailgun_domain,
                            &server_state.mailgun_api_key,
                            &id,
                            &format!("Error notifying of result: {:?}", err),
                            email_to_notify,
                        )
                        .await;
                    }
                });
                (StatusCode::OK, "Received")
            }
            Ok(state) => {
                trace!(%id, %event, "Received updated data");
                sessions.insert(
                    id,
                    Session {
                        state,
                        created,
                        email_to_notify,
                        id,
                    },
                );
                (StatusCode::OK, "Received")
            }
            Err(err) => {
                if let Some(state) = err.get_state() {
                    error!(%id, ?err, recoverable = true, "Error processing data");
                    sessions.insert(
                        id,
                        Session {
                            state: state.clone(),
                            created,
                            email_to_notify,
                            id,
                        },
                    );
                } else {
                    error!(%id, ?err, recoverable = false, "Error processing data");
                    tokio::spawn(async move {
                        let _ = notify_error(
                            &server_state.sender_email,
                            &server_state.mailgun_domain,
                            &server_state.mailgun_api_key,
                            &id,
                            &format!("Irrecoverable error processing data: {:?}", err),
                            email_to_notify,
                        )
                        .await;
                    });
                }
                (StatusCode::BAD_REQUEST, "Failed to process data")
            }
        }
    } else {
        error!(%id, "Received data for unknown ID");
        (StatusCode::NOT_FOUND, "Unknown ID")
    }
}

async fn print_json_data(Json(payload): Json<serde_json::Value>) -> String {
    let mut map = BTreeMap::new();
    let timestamp = Timestamp::now();
    map.insert(
        "timestamp",
        serde_json::Value::String(timestamp.to_string()),
    );
    map.insert("message", payload);
    println!("{}", serde_json::to_string(&map).unwrap());
    "Received".to_owned()
}

async fn graph_data(
    Path(id): Path<Uuid>,
    axum::extract::State(server_state): axum::extract::State<ServerState>,
) -> impl IntoResponse + Send {
    let sessions = server_state.sessions.lock().unwrap();
    if let Some(session) = sessions.get(&id) {
        match session.state.current_graph_png() {
            Ok(Some(bytes)) => (
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "image/png")],
                bytes,
            ),
            Ok(None) => {
                (
                    StatusCode::NOT_FOUND,
                    [(axum::http::header::CONTENT_TYPE, "text/plain")],
                    "No data has been received for this test yet. Please refresh the page when you expect there to be data.".as_bytes().to_owned(),
                )
            },
            Err(err) => {
                error!(?err, "Error generating graph for display");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(axum::http::header::CONTENT_TYPE, "text/plain")],
                    "Sorry, an error occurred".as_bytes().to_owned(),
                )
            }
        }
    } else {
        (
            StatusCode::NOT_FOUND,
            [(axum::http::header::CONTENT_TYPE, "text/plain")],
            "This test ID was not recognised. Either it has not been registered, or the test has already finished.".as_bytes().to_owned(),
        )
    }
}

async fn get_data_dummy(
    Path(id): Path<Uuid>,
    axum::extract::State(server_state): axum::extract::State<ServerState>,
) -> impl IntoResponse {
    if server_state.sessions.lock().unwrap().get(&id).is_some() {
        (
            StatusCode::OK,
            "This link is only intended to be used as a webhook. In the virus.sucks app, open 'Settings' and put it in the 'Webhook URL' field.",
        )
    } else {
        (StatusCode::NOT_FOUND, "Unknown ID")
    }
}

async fn count_sessions(
    axum::extract::State(server_state): axum::extract::State<ServerState>,
) -> impl IntoResponse + Send {
    let sessions = server_state.sessions.lock().unwrap();
    format!("{}", sessions.len())
}
