pub mod pb {
    tonic::include_proto!("rs.tokio.console.trace");
}

impl From<tracing_core::Level> for pb::metadata::Level {
    fn from(level: tracing_core::Level) -> Self {
        match level {
            tracing_core::Level::ERROR => pb::metadata::Level::Error,
            tracing_core::Level::WARN => pb::metadata::Level::Warn,
            tracing_core::Level::INFO => pb::metadata::Level::Info,
            tracing_core::Level::DEBUG => pb::metadata::Level::Debug,
            tracing_core::Level::TRACE => pb::metadata::Level::Trace,
            x => unreachable!("invalid level {:?}", x),
        }
    }
}

impl From<tracing_core::metadata::Kind> for pb::metadata::Kind {
    fn from(kind: tracing_core::metadata::Kind) -> Self {
        match kind {
            tracing_core::metadata::Kind::SPAN => pb::metadata::Kind::Span,
            tracing_core::metadata::Kind::EVENT => pb::metadata::Kind::Event,
            x => unreachable!("invalid metadata kind {:?}", x),
        }
    }
}

impl<'a> From<&'a tracing_core::Metadata<'a>> for pb::Metadata {
    fn from(meta: &'a tracing_core::Metadata<'a>) -> Self {
        pb::Metadata {
            name: meta.name().to_string(),
            target: meta.target().to_string(),
            file: self.file().unwrap_or("").to_string(),
            line: self.line().unwrap_or(0),
            column: self.column().unwrap_or(0),
            kind: pb::metadata::Kind::from(self.kind()) as i32,
            level: pb::metadata::Level::from(self.level()) as i32,
            ..Default::default()
        }
    }
}

impl From<&'static tracing_core::Metadata<'static>> for pb::MetaId {
    fn from(meta: &'static tracing_core::Metadata) -> Self {
        pb::MetaId {
            id: meta as *const _ as u64,
        }
    }
}

impl From<&'static tracing_core::Metadata<'static>>
    for pb::trace_event::register_metadata::NewMetadata
{
    fn from(meta: &'static tracing_core::Metadata) -> Self {
        pb::trace_event::register_metadata::NewMetadata {
            id: meta.into(),
            metadata: meta.into(),
        }
    }
}
