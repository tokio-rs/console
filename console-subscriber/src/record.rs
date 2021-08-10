use serde::{
    ser::{SerializeSeq, SerializeStruct},
    Serialize,
};
use std::{
    fs::File,
    io,
    path::Path,
    sync::{Arc, Mutex},
    time::SystemTime,
};

use console_api as proto;

/// This marks the currently understood version of the recording format. This
/// should be increased whenever the format has a breaking change that we
/// cannot parse. Though, even better, we should probably support parsing
/// older versions.
///
/// But while this is in rapid development, we can move fast and break things.
const DATA_FORMAT_VERSION: u8 = 1;

pub(crate) struct Recorder {
    buf: Arc<Mutex<RecordBuf>>,

    worker: std::thread::JoinHandle<()>,
}

struct Io {
    buf: Arc<Mutex<RecordBuf>>,
    file: File,
}

struct RecordBuf {
    /// The current buffer to serialize events into.
    bytes: Vec<u8>,
    /// The "next" buffer that should be used when the IO thread takes the
    /// current buffer. After flushing, the IO thread will put the buffer
    /// back in this slot, so the allocation can be reused.
    next: Vec<u8>,
}

#[derive(Serialize)]
struct Header {
    v: u8,
}

#[derive(Serialize)]
enum Event<'a> {
    Spawn {
        id: u64,
        at: SystemTime,
        fields: SerializeFields<'a>,
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

struct SerializeFields<'a>(&'a [proto::Field]);

struct SerializeField<'a>(&'a proto::Field);

impl Recorder {
    pub(crate) fn new(path: &Path) -> io::Result<Self> {
        let buf = Arc::new(Mutex::new(RecordBuf::new()));
        let buf2 = buf.clone();
        let file = std::fs::File::create(path)?;

        let worker = std::thread::Builder::new()
            .name("console/subscriber/recorder/io".into())
            .spawn(move || {
                record_io(Io { buf: buf2, file });
            })?;

        let recorder = Recorder { buf, worker };

        recorder.write(&Header {
            v: DATA_FORMAT_VERSION,
        });

        Ok(recorder)
    }

    pub(crate) fn record(&self, event: &crate::Event) {
        let event = match event {
            crate::Event::Spawn { id, at, fields, .. } => Event::Spawn {
                id: id.into_u64(),
                at: *at,
                fields: SerializeFields(fields),
            },
            crate::Event::Enter { id, at } => Event::Enter {
                id: id.into_u64(),
                at: *at,
            },
            crate::Event::Exit { id, at } => Event::Exit {
                id: id.into_u64(),
                at: *at,
            },
            crate::Event::Close { id, at } => Event::Close {
                id: id.into_u64(),
                at: *at,
            },
            crate::Event::Waker { id, op, at } => Event::Waker {
                id: id.into_u64(),
                at: *at,
                op: *op,
            },
            _ => return,
        };

        self.write(&event);
    }

    fn write<T: Serialize>(&self, val: &T) {
        let mut buf = self.buf.lock().unwrap();
        serde_json::to_writer(&mut buf.bytes, val).expect("json");
        buf.bytes.push(b'\n');
        drop(buf);
        self.worker.thread().unpark();
    }
}

impl RecordBuf {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            next: Vec::new(),
        }
    }

    /// Takes the existing bytes to be written, and resets self so that
    /// it may continue to buffer events.
    fn take(&mut self) -> Vec<u8> {
        let next = std::mem::take(&mut self.next);
        std::mem::replace(&mut self.bytes, next)
    }

    fn put(&mut self, mut next: Vec<u8>) {
        debug_assert_eq!(self.next.capacity(), 0);
        next.clear();
        self.next = next;
    }
}

fn record_io(mut dst: Io) {
    use std::io::Write;

    loop {
        std::thread::park();

        // Only lock the mutex to take the bytes out. The file write could
        // take a relatively long time, and we don't want to be blocking
        // the serialization end holding this lock.
        let bytes = dst.buf.lock().unwrap().take();
        match dst.file.write_all(&bytes) {
            Ok(()) => {
                dst.buf.lock().unwrap().put(bytes);
            }
            Err(_e) => {
                // TODO: what to do if file error?
            }
        }
    }
}

impl serde::Serialize for SerializeFields<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for element in self.0 {
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
