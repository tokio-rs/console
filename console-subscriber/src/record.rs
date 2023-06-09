use console_api as proto;
use crossbeam_channel::{Receiver, Sender};
use serde::{
    ser::{SerializeSeq, SerializeStruct},
    Serialize,
};
use std::{fs::File, io, path::Path, time::SystemTime};

/// This marks the currently understood version of the recording format. This
/// should be increased whenever the format has a breaking change that we
/// cannot parse. Though, even better, we should probably support parsing
/// older versions.
///
/// But while this is in rapid development, we can move fast and break things.
const DATA_FORMAT_VERSION: u8 = 1;

pub(crate) struct Recorder {
    tx: Sender<Event>,
    // TODO(eliza): terminate and flush when dropping...
    _worker: std::thread::JoinHandle<()>,
}

#[derive(Serialize)]
struct Header {
    v: u8,
}

#[derive(Serialize)]
pub(crate) enum Event {
    Spawn {
        id: u64,
        at: SystemTime,
        fields: SerializeFields,
    },
    Enter {
        id: u64,
        at: SystemTime,
    },
    Exit {
        id: u64,
        at: SystemTime,
    },
    Close {
        id: u64,
        at: SystemTime,
    },
    Waker {
        id: u64,
        op: super::WakeOp,
        at: SystemTime,
    },
}

pub(crate) struct SerializeFields(pub(crate) Vec<proto::Field>);

struct SerializeField<'a>(&'a proto::Field);

impl Recorder {
    pub(crate) fn new(path: &Path) -> io::Result<Self> {
        let file = std::fs::File::create(path)?;
        let (tx, rx) = crossbeam_channel::bounded(4096);
        let _worker = std::thread::Builder::new()
            .name("console/subscriber/recorder/io".into())
            .spawn(move || {
                if let Err(e) = record_io(file, rx) {
                    eprintln!("event recorder failed: {}", e);
                }
            })?;

        let recorder = Recorder { tx, _worker };

        Ok(recorder)
    }

    pub(crate) fn record(&self, event: Event) {
        if self.tx.send(event).is_err() {
            eprintln!("event recorder thread has terminated!");
        }
    }
}

fn record_io(file: File, rx: Receiver<Event>) -> io::Result<()> {
    use std::io::{BufWriter, Write};

    fn write<T: Serialize>(mut file: &mut BufWriter<File>, val: &T) -> io::Result<()> {
        serde_json::to_writer(&mut file, val)?;
        file.write_all(b"\n")
    }

    let mut file = BufWriter::new(file);
    write(
        &mut file,
        &Header {
            v: DATA_FORMAT_VERSION,
        },
    )?;

    // wait to receive an event...
    while let Ok(event) = rx.recv() {
        // TODO: what to do if file error?
        write(&mut file, &event)?;

        // drain any additional events that are ready now
        while let Ok(event) = rx.try_recv() {
            write(&mut file, &event)?;
        }

        file.flush()?;
    }

    tracing::debug!("event stream ended; flushing file");
    file.flush()
}

impl serde::Serialize for SerializeFields {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for element in &self.0 {
            seq.serialize_element(&SerializeField(element))?;
        }
        seq.end()
    }
}

impl serde::Serialize for SerializeField<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut ser = serializer.serialize_struct("Field", 2)?;
        ser.serialize_field(
            "name",
            match self.0.name.as_ref().expect("name") {
                proto::field::Name::StrName(ref n) => n,
                proto::field::Name::NameIdx(_idx) => todo!("metadata idx"),
            },
        )?;

        match self.0.value.as_ref().expect("field value") {
            proto::field::Value::DebugVal(v) | proto::field::Value::StrVal(v) => {
                ser.serialize_field("value", v)?;
            }
            proto::field::Value::U64Val(v) => {
                ser.serialize_field("value", v)?;
            }
            proto::field::Value::I64Val(v) => {
                ser.serialize_field("value", v)?;
            }
            proto::field::Value::BoolVal(v) => {
                ser.serialize_field("value", v)?;
            }
        }
        ser.end()
    }
}
