use std::{
    collections::HashMap,
    str::FromStr,
    sync::{Arc, Mutex},
    time::Duration,
};

use email_address::EmailAddress;
use jiff::Timestamp;
use tracing::info;
use uuid::Uuid;

use crate::{Error, state::State};

#[derive(Clone)]
pub struct ServerState {
    pub sessions: Arc<Mutex<Sessions>>,
    pub base_url: String,
    pub sender_email: EmailAddress,
    pub mailgun_domain: String,
    pub mailgun_api_key: String,
    pub cleanup_period: Duration,
}

impl ServerState {
    pub fn try_from_env() -> Result<ServerState, Error> {
        let base_url = Self::env_var("BASE_URL")?;
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
}
