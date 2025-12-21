use std::{
    collections::HashMap,
    str::FromStr,
    sync::{Arc, Mutex},
    time::Duration,
};

use email_address::EmailAddress;
use jiff::Timestamp;
use prometheus::{Histogram, HistogramOpts, HistogramVec, Opts, Registry};
use tracing::info;
use uuid::Uuid;

use crate::{Error, state::State, websockets::SessionSockets};

#[derive(Clone)]
pub struct ServerState {
    pub sessions: Arc<Mutex<Sessions>>,
    pub base_url: String,
    pub websocket_base_url: String,
    pub sender_email: EmailAddress,
    pub mailgun_domain: String,
    pub mailgun_api_key: String,
    pub cleanup_period: Duration,
}

impl ServerState {
    pub fn try_from_env() -> Result<ServerState, Error> {
        let base_url = Self::env_var("BASE_URL")?;
        let websocket_base_url = if let Some(suffix) = base_url.strip_prefix("http") {
            "ws".to_string() + suffix
        } else {
            return Err(Error::InvalidEnvVar {
                name: "BASE_URL".to_owned(),
                cause: format!(
                    "Expected BASE_URL to start with http but was '{}'",
                    base_url
                )
                .into(),
            });
        };
        let sender_email = Self::env_var("SENDER_EMAIL")?;
        let sender_email =
            EmailAddress::from_str(&sender_email).map_err(|err| Error::InvalidEnvVar {
                name: "SENDER_EMAIL".to_owned(),
                cause: Box::new(err),
            })?;
        let mailgun_domain = Self::env_var("MAILGUN_DOMAIN")?;
        let mailgun_api_key = Self::env_var("MAILGUN_API_KEY")?;
        let cleanup_period = Self::env_var("CLEANUP_PERIOD")?;
        let cleanup_period =
            duration_str::parse(&cleanup_period).map_err(|err| Error::InvalidEnvVar {
                name: "CLEANUP_PERIOD".to_owned(),
                cause: format!("Failed to parse duration {}: {}", cleanup_period, err).into(),
            })?;
        Ok(ServerState {
            sessions: Arc::new(Mutex::new(Sessions::default())),
            base_url,
            websocket_base_url,
            sender_email,
            mailgun_domain,
            mailgun_api_key,
            cleanup_period,
        })
    }

    fn env_var(name: &str) -> Result<String, Error> {
        std::env::var(name).map_err(|err| Error::InvalidEnvVar {
            name: name.to_owned(),
            cause: Box::new(err),
        })
    }

    pub fn create_session(&self, email_to_notify: EmailAddress) -> Uuid {
        let id = {
            let mut sessions = self.sessions.lock().unwrap();
            sessions.create(email_to_notify)
        };
        let sessions = self.sessions.clone();
        let cleanup_period = self.cleanup_period;
        tokio::spawn(async move {
            tokio::time::sleep(cleanup_period).await;
            let mut sessions = sessions.lock().unwrap();
            if let Some(removed) = sessions.remove(&id) {
                info!("Expired session {}", removed.id);
            }
        });
        id
    }

    pub fn get_metrics(&self) -> Registry {
        let registry = Registry::new();
        let session_duration = HistogramVec::new(
            HistogramOpts {
                common_opts: Opts {
                    namespace: "web".to_owned(),
                    subsystem: "session".to_owned(),
                    name: "duration".to_owned(),
                    help: "Time since session was created".to_owned(),
                    const_labels: HashMap::new(),
                    variable_labels: vec!["has_data".to_owned()],
                },
                buckets: Self::time_buckets(),
            },
            &["has_data"],
        )
        .expect("Metric should never fail to be created");
        registry
            .register(Box::new(session_duration.clone()))
            .expect("Fresh registry should never fail to register metric");

        let time_to_first_result = Histogram::with_opts(HistogramOpts {
            common_opts: Opts {
                namespace: "web".to_owned(),
                subsystem: "session".to_owned(),
                name: "time_to_first_result".to_owned(),
                help: "Time to first result in a session".to_owned(),
                const_labels: HashMap::new(),
                variable_labels: vec![],
            },
            buckets: Self::time_buckets(),
        })
        .expect("Metric should never fail to be created");
        registry
            .register(Box::new(time_to_first_result.clone()))
            .expect("Fresh registry should never fail to register metric");

        let now = Timestamp::now();
        let sessions = self.sessions.lock().unwrap();
        for session in sessions.states.values() {
            let duration_to_first_result = session.state.duration_to_first_result();
            session_duration
                .with_label_values(&[&format!("{}", duration_to_first_result.is_some())])
                .observe(now.duration_since(session.created).as_secs_f64());
            if let Some(duration_to_first_result) = duration_to_first_result {
                time_to_first_result.observe(duration_to_first_result.as_secs_f64());
            }
        }
        registry
    }

    fn time_buckets() -> Vec<f64> {
        vec![
            10_f64,
            30_f64,
            60_f64,
            90_f64,
            120_f64,
            150_f64,
            180_f64,
            210_f64,
            240_f64,
            270_f64,
            300_f64,
            330_f64,
            360_f64,
            390_f64,
            420_f64,
            450_f64,
            480_f64,
            510_f64,
            540_f64,
            570_f64,
            600_f64,
            900_f64,
            1200_f64,
            1800_f64,
            40_f64 * 60_f64,
            60_f64 * 60_f64,
            2_f64 * 60_f64 * 60_f64,
            3_f64 * 60_f64 * 60_f64,
            6_f64 * 60_f64 * 60_f64,
            12_f64 * 60_f64 * 60_f64,
            24_f64 * 60_f64 * 60_f64,
            2_f64 * 24_f64 * 60_f64 * 60_f64,
            3_f64 * 24_f64 * 60_f64 * 60_f64,
            4_f64 * 24_f64 * 60_f64 * 60_f64,
            5_f64 * 24_f64 * 60_f64 * 60_f64,
            6_f64 * 24_f64 * 60_f64 * 60_f64,
        ]
    }
}

#[derive(Default)]
pub struct Sessions {
    states: HashMap<Uuid, Session>,
}

#[allow(clippy::len_without_is_empty)]
impl Sessions {
    fn create(&mut self, email_to_notify: EmailAddress) -> Uuid {
        let id = Uuid::new_v4();
        let timestamp = Timestamp::now();
        let session = Session {
            state: State::started(),
            created: timestamp,
            email_to_notify,
            id,
            websockets: SessionSockets::new(),
        };
        self.insert(id, session);
        id
    }

    pub fn get(&self, id: &Uuid) -> Option<&Session> {
        self.states.get(id)
    }

    pub fn remove(&mut self, id: &Uuid) -> Option<Session> {
        self.states.remove(id)
    }

    pub fn insert(&mut self, id: Uuid, session: Session) {
        self.states.insert(id, session);
    }

    pub fn len(&self) -> usize {
        self.states.len()
    }
}

pub struct Session {
    pub state: State,
    pub created: Timestamp,
    pub email_to_notify: EmailAddress,
    pub id: Uuid,
    pub websockets: SessionSockets,
}
