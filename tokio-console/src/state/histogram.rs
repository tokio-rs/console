use console_api::tasks as proto;
use hdrhistogram::Histogram;
use std::{io::Cursor, time::Duration};

#[derive(Debug)]
pub(crate) struct DurationHistogram {
    pub(crate) histogram: Histogram<u64>,
    pub(crate) high_outliers: u64,
    pub(crate) highest_outlier: Option<Duration>,
}

impl DurationHistogram {
    pub(crate) fn from_poll_durations(
        proto: &proto::task_details::PollTimesHistogram,
    ) -> Option<Self> {
        match proto {
            proto::task_details::PollTimesHistogram::Histogram(hist) => Self::from_proto(hist),
            proto::task_details::PollTimesHistogram::LegacyHistogram(bytes) => {
                Self::from_proto_legacy(&bytes[..])
            }
        }
    }

    fn from_proto_legacy(bytes: &[u8]) -> Option<Self> {
        let histogram = deserialize_histogram(bytes)?;
        Some(Self {
            histogram,
            high_outliers: 0,
            highest_outlier: None,
        })
    }

    pub(crate) fn from_proto(proto: &proto::DurationHistogram) -> Option<Self> {
        let histogram = deserialize_histogram(&proto.raw_histogram[..])?;
        Some(Self {
            histogram,
            high_outliers: proto.high_outliers,
            highest_outlier: proto.highest_outlier.map(Duration::from_nanos),
        })
    }
}

fn deserialize_histogram(bytes: &[u8]) -> Option<Histogram<u64>> {
    hdrhistogram::serialization::Deserializer::new()
        .deserialize(&mut Cursor::new(&bytes))
        .ok()
}
