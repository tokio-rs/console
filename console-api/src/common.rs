use std::fmt;

tonic::include_proto!("rs.tokio.console.common");

impl From<tracing_core::Level> for metadata::Level {
    fn from(level: tracing_core::Level) -> Self {
        match level {
            tracing_core::Level::ERROR => metadata::Level::Error,
            tracing_core::Level::WARN => metadata::Level::Warn,
            tracing_core::Level::INFO => metadata::Level::Info,
            tracing_core::Level::DEBUG => metadata::Level::Debug,
            tracing_core::Level::TRACE => metadata::Level::Trace,
        }
    }
}

impl From<tracing_core::metadata::Kind> for metadata::Kind {
    fn from(kind: tracing_core::metadata::Kind) -> Self {
        match kind {
            tracing_core::metadata::Kind::SPAN => metadata::Kind::Span,
            tracing_core::metadata::Kind::EVENT => metadata::Kind::Event,
        }
    }
}

impl<'a> From<&'a tracing_core::Metadata<'a>> for Metadata {
    fn from(meta: &'a tracing_core::Metadata<'a>) -> Self {
        let kind = if meta.is_span() {
            metadata::Kind::Span
        } else {
            debug_assert!(meta.is_event());
            metadata::Kind::Event
        };

        let location = Location {
            file: meta.file().map(String::from),
            module_path: meta.module_path().map(String::from),
            line: meta.line(),
            column: None, // tracing doesn't support columns yet
        };

        Metadata {
            name: meta.name().to_string(),
            target: meta.target().to_string(),
            location: Some(location),
            kind: kind as i32,
            level: metadata::Level::from(*meta.level()) as i32,
            field_names: Vec::new(),
            ..Default::default()
        }
    }
}

impl<'a> From<&'a std::panic::Location<'a>> for Location {
    fn from(loc: &'a std::panic::Location<'a>) -> Self {
        Location {
            file: Some(loc.file().to_string()),
            line: Some(loc.line()),
            column: Some(loc.column()),
            ..Default::default()
        }
    }
}

impl fmt::Display for field::Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            field::Value::BoolVal(v) => fmt::Display::fmt(v, f)?,
            field::Value::StrVal(v) => fmt::Display::fmt(v, f)?,
            field::Value::U64Val(v) => fmt::Display::fmt(v, f)?,
            field::Value::DebugVal(v) => fmt::Display::fmt(v, f)?,
            field::Value::I64Val(v) => fmt::Display::fmt(v, f)?,
        }

        Ok(())
    }
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name_val = (self.name.as_ref(), self.value.as_ref());
        if let (Some(field::Name::StrName(name)), Some(val)) = name_val {
            write!(f, "{}={}", name, val)?;
        }

        Ok(())
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.module_path.as_ref(), self.file.as_ref()) {
            // Module paths take predecence because they're shorter...
            (Some(module), _) => f.write_str(module.as_ref())?,
            (None, Some(file)) => f.write_str(file.as_ref())?,
            // If there's no file or module path, then printing the line and
            // column makes no sense...
            (None, None) => return f.write_str("<unknown location>"),
        };

        if let Some(line) = self.line {
            write!(f, ":{}", line)?;

            // Printing the column only makes sense if there's a line...
            if let Some(column) = self.column {
                write!(f, ":{}", column)?;
            }
        }

        Ok(())
    }
}

// === IDs ===

impl From<&'static tracing_core::Metadata<'static>> for MetaId {
    fn from(meta: &'static tracing_core::Metadata) -> Self {
        MetaId {
            id: meta as *const _ as u64,
        }
    }
}

impl From<tracing_core::span::Id> for SpanId {
    fn from(id: tracing_core::span::Id) -> Self {
        SpanId { id: id.into_u64() }
    }
}

impl From<&'static tracing_core::Metadata<'static>> for register_metadata::NewMetadata {
    fn from(meta: &'static tracing_core::Metadata) -> Self {
        register_metadata::NewMetadata {
            id: Some(meta.into()),
            metadata: Some(meta.into()),
        }
    }
}

impl From<i64> for field::Value {
    fn from(val: i64) -> Self {
        field::Value::I64Val(val)
    }
}

impl From<u64> for field::Value {
    fn from(val: u64) -> Self {
        field::Value::U64Val(val)
    }
}

impl From<bool> for field::Value {
    fn from(val: bool) -> Self {
        field::Value::BoolVal(val)
    }
}

impl From<&str> for field::Value {
    fn from(val: &str) -> Self {
        field::Value::StrVal(val.into())
    }
}

impl From<&str> for field::Name {
    fn from(val: &str) -> Self {
        field::Name::StrName(val.into())
    }
}

impl From<&dyn std::fmt::Debug> for field::Value {
    fn from(val: &dyn std::fmt::Debug) -> Self {
        field::Value::DebugVal(format!("{:?}", val))
    }
}
