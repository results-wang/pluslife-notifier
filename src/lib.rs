use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use crate::{messages::Message, state::State};

pub mod graph;
pub mod mailgun;
pub mod messages;
pub mod notifier;
pub mod sessions;
pub mod state;
pub mod websockets;

#[derive(Debug)]
pub enum Error {
    TestFinishedMissingResult,
    MissingTestFinished(State),
    UnexpectedMessage(State, Box<Message>),
    TooManyChannels(usize),

    InvalidEnvVar {
        name: String,
        cause: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    Io(std::io::Error),
    Serde(serde_json::Error),
    Plotting(plotters::drawing::DrawingAreaErrorKind<plotters_bitmap::BitMapBackendError>),
    Reqwest(reqwest::Error),
}

impl Error {
    pub fn get_state(&self) -> Option<&State> {
        match self {
            Error::TestFinishedMissingResult => None,
            Error::MissingTestFinished(state) => Some(state),
            Error::UnexpectedMessage(state, _) => Some(state),
            Error::TooManyChannels(_) => None,
            Error::InvalidEnvVar { .. } => None,
            Error::Io(_) => None,
            Error::Serde(_) => None,
            Error::Plotting(_) => None,
            Error::Reqwest(_) => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Serde(err)
    }
}

impl From<plotters::drawing::DrawingAreaErrorKind<plotters_bitmap::BitMapBackendError>> for Error {
    fn from(
        err: plotters::drawing::DrawingAreaErrorKind<plotters_bitmap::BitMapBackendError>,
    ) -> Self {
        Error::Plotting(err)
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Error::Reqwest(err)
    }
}

#[derive(Deserialize, Serialize)]
pub struct LogWrapper {
    pub timestamp: Timestamp,
    pub message: Message,
}
