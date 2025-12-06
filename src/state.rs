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
        State::IncompleteTest(IncompleteTest::new(TestData::empty()))
    }

    pub fn update(self, message: Message, websockets: &SessionSockets) -> Result<State, Error> {
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
                    let new_state = State::incomplete(message.test.data);
                    websockets.notify(&new_state);
                    Ok(new_state)
                }
                Event::DeviceReady => Ok(State::incomplete(message.test.data)),
                Event::TestStarted => Ok(State::incomplete(message.test.data)),
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

    fn incomplete(data: TestData) -> State {
        State::IncompleteTest(IncompleteTest::new(data))
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
}

#[derive(Clone, Debug)]
pub struct IncompleteTest {
    pub data: TestData,
}

impl IncompleteTest {
    pub fn new(data: TestData) -> IncompleteTest {
        IncompleteTest { data }
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
