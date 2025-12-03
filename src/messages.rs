use jiff::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub version: u8,
    pub event: Event,
    pub device: Device,
    pub test: Test,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, strum_macros::Display, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(deny_unknown_fields)]
pub enum Event {
    TestStarted,
    ContinueTest,
    TestFinished,
    NewData,
    DeviceReady,
    AlreadyTesting,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Device {
    #[serde(rename = "hwVersion")]
    pub hardware_version: String,
    #[serde(rename = "swVersion")]
    pub software_version: String,
    #[serde(rename = "deviceModel")]
    pub device_model: String,
    #[serde(rename = "sn")]
    pub serial_number: u64,
    pub configuration: String,

    #[serde(rename = "currentTemp")]
    pub current_temp: Option<DegreesC>,
    #[serde(rename = "targetTemp")]
    pub target_temp: Option<DegreesC>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Test {
    pub data: TestData,
    pub state: TestState,
    pub result: Option<TestResult>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TestData {
    pub samples: Vec<TestSample>,
    #[serde(rename = "temperatureSamples")]
    pub temperature_samples: Vec<TemperatureSample>,
}

impl TestData {
    pub fn empty() -> TestData {
        TestData {
            samples: Vec::new(),
            temperature_samples: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TestSample {
    // TODO: What is this relative to? What's its scope/range?
    #[serde(rename = "currentDataIndex")]
    pub current_data_index: usize,

    // TODO: Why first?
    #[serde(rename = "firstChannelResult")]
    pub first_channel_result: i64,

    // TODO: Is this ever not 1?
    #[serde(rename = "numberOfChannels")]
    pub number_of_channels: usize,

    // TODO: What is this?
    #[serde(rename = "sampleStreamNumber")]
    pub sample_stream_number: i64,

    // TODO: What is this? It seems to always be 1?
    #[serde(rename = "sampleType")]
    pub sample_type: i64,

    #[serde(rename = "samplingTemperature")]
    pub sampling_temperature: DegreesC,

    // TODO: Units? Relative to?
    // Looks like it's hundred-milliseconds since start of test.
    #[serde(rename = "samplingTime")]
    pub sampling_time: u64,

    // TODO: Why starting?
    #[serde(rename = "startingChannel")]
    pub starting_channel: usize,

    // TODO: Why?
    #[serde(rename = "totalNumberOfSamples")]
    pub total_number_of_samples: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TemperatureSample {
    pub time: Timestamp,
    pub temp: DegreesC,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(deny_unknown_fields)]
pub enum TestState {
    Idle,
    Testing,
    Done,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TestResult {
    // TODO: What is this?
    #[serde(rename = "detectionType")]
    pub detection_type: i64,

    // Maybe same as sampleStreamNumber?
    #[serde(rename = "detectionFlowNumber")]
    pub detection_flow_number: i64,

    #[serde(rename = "detectionResult")]
    pub detection_result: DetectionResult,

    // TODO: Is this ever not 7?
    #[serde(rename = "numberOfChannels")]
    pub number_of_channels: usize,

    // TODO: Is this ever not 0?
    #[serde(rename = "startingChannel")]
    pub starting_channel: usize,

    #[serde(rename = "channelResults")]
    pub channel_results: Vec<DetectionResult>,

    #[serde(rename = "numberOfSubGroups")]
    pub number_of_subgroups: usize,

    #[serde(rename = "subGroupResults")]
    pub subgroup_results: Vec<SubgroupResult>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SubgroupResult {
    pub name: String,
    pub result: DetectionResult,
}

#[derive(Clone, Copy, Debug, Deserialize, strum_macros::Display, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(deny_unknown_fields)]
pub enum DetectionResult {
    Positive,
    Negative,
    Invalid,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, transparent)]
pub struct DegreesC(pub f64);

#[cfg(test)]
mod test {
    use super::Event;

    #[test]
    fn event_from_str() {
        assert_eq!(
            Event::TestStarted,
            serde_json::from_str("\"TEST_STARTED\"").unwrap()
        );
        assert_eq!(
            Event::NewData,
            serde_json::from_str("\"NEW_DATA\"").unwrap()
        );
    }
}
