use jiff::{SignedDuration, Timestamp};

use crate::{
    Error,
    messages::{DetectionResult, Event, Message, SubgroupResult, TestData, TestResult},
    websockets::SessionSockets,
};

#[derive(Clone, Debug)]
pub enum State {
    IncompleteTest(IncompleteTest),
    CompletedTest(CompletedTest),
}

impl State {
    pub fn started() -> State {
        State::IncompleteTest(IncompleteTest::new(TestData::empty(), None))
    }

    pub fn update(
        self,
        message: Message,
        session_creation_time: Timestamp,
        websockets: &SessionSockets,
    ) -> Result<State, Error> {
        let duration_to_first_result =
            self.duration_to_first_result_or_since(session_creation_time);
        match self {
            State::IncompleteTest(incomplete_test) => match message.event {
                Event::TestFinished => {
                    if let Some(result) = message.test.result {
                        let completed_test = incomplete_test.complete(result, message.test.data)?;
                        let new_state = State::CompletedTest(completed_test);
                        websockets.notify(&new_state);
                        Ok(new_state)
                    } else {
                        Err(Error::TestFinishedMissingResult)
                    }
                }
                Event::NewData => {
                    let new_state = State::incomplete(message.test.data, duration_to_first_result);
                    websockets.notify(&new_state);
                    Ok(new_state)
                }
                Event::DeviceReady => Ok(State::incomplete(
                    message.test.data,
                    duration_to_first_result,
                )),
                Event::TestStarted => Ok(State::incomplete(
                    message.test.data,
                    duration_to_first_result,
                )),
                Event::AlreadyTesting | Event::ContinueTest => Err(Error::UnexpectedMessage(
                    State::IncompleteTest(incomplete_test),
                    Box::new(message),
                )),
            },
            State::CompletedTest(completed_test) => Err(Error::UnexpectedMessage(
                State::CompletedTest(completed_test),
                Box::new(message),
            )),
        }
    }

    fn incomplete(data: TestData, duration_to_first_result: SignedDuration) -> State {
        State::IncompleteTest(IncompleteTest::new(data, Some(duration_to_first_result)))
    }

    pub fn current_graph_png(&self) -> Result<Option<Vec<u8>>, Error> {
        match self {
            State::IncompleteTest(test) => {
                if test.data.samples.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(
                        test.data
                            .to_graph()?
                            .normalise_values_to_zero()
                            .plot_to_buffer()?,
                    ))
                }
            }
            State::CompletedTest(test) => Ok(Some(test.graph_png.clone())),
        }
    }

    pub fn duration_to_first_result(&self) -> Option<SignedDuration> {
        match self {
            State::IncompleteTest(test) => test.duration_to_first_result,
            Self::CompletedTest(_) => None,
        }
    }

    pub fn duration_to_first_result_or_since(&self, timestamp: Timestamp) -> SignedDuration {
        self.duration_to_first_result()
            .unwrap_or_else(|| Timestamp::now().duration_since(timestamp))
    }
}

#[derive(Clone, Debug)]
pub struct IncompleteTest {
    pub data: TestData,
    pub duration_to_first_result: Option<SignedDuration>,
}

impl IncompleteTest {
    pub fn new(data: TestData, duration_to_first_result: Option<SignedDuration>) -> IncompleteTest {
        IncompleteTest {
            data,
            duration_to_first_result,
        }
    }

    pub fn complete(self, result: TestResult, data: TestData) -> Result<CompletedTest, Error> {
        Ok(CompletedTest {
            overall: result.detection_result,
            subgroup_results: result.subgroup_results,
            graph_png: data
                .to_graph()?
                .normalise_values_to_zero()
                .plot_to_buffer()?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct CompletedTest {
    pub overall: DetectionResult,
    pub subgroup_results: Vec<SubgroupResult>,
    pub graph_png: Vec<u8>,
}
