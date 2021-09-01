//! These visitors are responsible for extracting the relevant
//! fields from tracing metadata and producing the parts
//! needed to construct `Event` instances.

use super::{AttributeUpdate, AttributeUpdateOp, WakeOp};
use console_api as proto;
use proto::resources::resource;
use tracing_core::{
    field::{self, Visit},
    span,
};

/// Used to extract the fields needed to construct
/// an Event::Resource from the metadata of a tracing span
/// that has the following shape:
///
/// tracing::trace_span!(
///     "runtime.resource",
///     concrete_type = "Sleep",
///     kind = "timer",
/// );
///
/// Fields:
/// concrete_type - indicates the concrete rust type for this resource
/// kind - indicates the type of resource (i.e. timer, sync, io )
#[derive(Default)]
pub(crate) struct ResourceVisitor {
    concrete_type: Option<String>,
    kind: Option<resource::Kind>,
}

/// Used to extract all fields from the metadata
/// of a tracing span
pub(crate) struct FieldVisitor {
    fields: Vec<proto::Field>,
    meta_id: proto::MetaId,
}

/// Used to extract the fields needed to construct
/// an Event::AsyncOp from the metadata of a tracing span
/// that has the following shape:
///
/// tracing::trace_span!(
///     "runtime.resource.async_op",
///     source = "Sleep::new_timeout",
/// );
///
/// Fields:
/// source - the method which has created an instance of this async operation
#[derive(Default)]
pub(crate) struct AsyncOpVisitor {
    source: Option<String>,
}

/// Used to extract the fields needed to construct
/// an Event::Waker from the metadata of a tracing span
/// that has the following shape:
///
/// tracing::trace!(
///     target: "tokio::task::waker",
///     op = "waker.clone",
///     task.id = id.into_u64(),
/// );
///
/// Fields:
/// task.id - the id of the task this waker will wake
/// op - the operation associated with this waker event
#[derive(Default)]
pub(crate) struct WakerVisitor {
    id: Option<span::Id>,
    op: Option<WakeOp>,
}

/// Used to extract the fields needed to construct
/// an Event::PollOp from the metadata of a tracing event
/// that has the following shape:
///
/// tracing::trace!(
///     target: "runtime::resource::poll_op",
///     op_name = "poll_elapsed",
///     readiness = "pending"
/// );
///
/// Fields:
/// op_name - the name of this resource poll operation
/// readiness - the result of invoking this poll op, describing its readiness
#[derive(Default)]
pub(crate) struct PollOpVisitor {
    op_name: Option<String>,
    is_ready: Option<bool>,
}

/// Used to extract the fields needed to construct
/// an Event::StateUpdate from the metadata of a tracing event
/// that has the following shape:
///
/// tracing::trace!(
///     target: "runtime::resource::state_update",
///     duration = duration,
///     duration.unit = "ms",
///     duration.op = "override",
/// );
///
/// Fields:
/// attribute_name - a field value for a field that has the name of the resource attribute being updated
/// value - the value for this update
/// unit - the unit for the value being updated (e.g. ms, s, bytes)
/// op - the operation that this update performs to the value of the resource attribute (one of: ovr, sub, add)
pub(crate) struct StateUpdateVisitor {
    meta_id: proto::MetaId,
    field: Option<proto::Field>,
    unit: Option<String>,
    op: Option<AttributeUpdateOp>,
}

impl ResourceVisitor {
    pub(crate) const RES_SPAN_NAME: &'static str = "runtime.resource";
    const RES_CONCRETE_TYPE_FIELD_NAME: &'static str = "concrete_type";
    const RES_KIND_FIELD_NAME: &'static str = "kind";
    const RES_KIND_TIMER: &'static str = "timer";

    pub(crate) fn result(self) -> Option<(String, resource::Kind)> {
        self.concrete_type.zip(self.kind)
    }
}

impl Visit for ResourceVisitor {
    fn record_debug(&mut self, _: &field::Field, _: &dyn std::fmt::Debug) {}

    fn record_str(&mut self, field: &tracing_core::Field, value: &str) {
        match field.name() {
            Self::RES_CONCRETE_TYPE_FIELD_NAME => self.concrete_type = Some(value.to_string()),
            Self::RES_KIND_FIELD_NAME => {
                let kind = Some(match value {
                    Self::RES_KIND_TIMER => {
                        resource::kind::Kind::Known(resource::kind::Known::Timer as i32)
                    }
                    other => resource::kind::Kind::Other(other.to_string()),
                });
                self.kind = Some(resource::Kind { kind });
            }
            _ => {}
        }
    }
}

impl FieldVisitor {
    pub(crate) fn new(meta_id: proto::MetaId) -> Self {
        FieldVisitor {
            fields: Vec::default(),
            meta_id,
        }
    }
    pub(crate) fn result(self) -> Vec<proto::Field> {
        self.fields
    }
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &field::Field, value: &dyn std::fmt::Debug) {
        self.fields.push(proto::Field {
            name: Some(field.name().into()),
            value: Some(value.into()),
            metadata_id: Some(self.meta_id.clone()),
        });
    }

    fn record_i64(&mut self, field: &tracing_core::Field, value: i64) {
        self.fields.push(proto::Field {
            name: Some(field.name().into()),
            value: Some(value.into()),
            metadata_id: Some(self.meta_id.clone()),
        });
    }

    fn record_u64(&mut self, field: &tracing_core::Field, value: u64) {
        self.fields.push(proto::Field {
            name: Some(field.name().into()),
            value: Some(value.into()),
            metadata_id: Some(self.meta_id.clone()),
        });
    }

    fn record_bool(&mut self, field: &tracing_core::Field, value: bool) {
        self.fields.push(proto::Field {
            name: Some(field.name().into()),
            value: Some(value.into()),
            metadata_id: Some(self.meta_id.clone()),
        });
    }

    fn record_str(&mut self, field: &tracing_core::Field, value: &str) {
        self.fields.push(proto::Field {
            name: Some(field.name().into()),
            value: Some(value.into()),
            metadata_id: Some(self.meta_id.clone()),
        });
    }
}

impl AsyncOpVisitor {
    pub(crate) const ASYNC_OP_SPAN_NAME: &'static str = "runtime.resource.async_op";
    const ASYNC_OP_SRC_FIELD_NAME: &'static str = "source";

    pub(crate) fn result(self) -> Option<String> {
        self.source
    }
}

impl Visit for AsyncOpVisitor {
    fn record_debug(&mut self, _: &field::Field, _: &dyn std::fmt::Debug) {}

    fn record_str(&mut self, field: &tracing_core::Field, value: &str) {
        if field.name() == Self::ASYNC_OP_SRC_FIELD_NAME {
            self.source = Some(value.to_string());
        }
    }
}

impl WakerVisitor {
    const WAKE: &'static str = "waker.wake";
    const WAKE_BY_REF: &'static str = "waker.wake_by_ref";
    const CLONE: &'static str = "waker.clone";
    const DROP: &'static str = "waker.drop";
    const TASK_ID_FIELD_NAME: &'static str = "task.id";

    pub(crate) fn result(self) -> Option<(span::Id, WakeOp)> {
        self.id.zip(self.op)
    }
}

impl Visit for WakerVisitor {
    fn record_debug(&mut self, _: &field::Field, _: &dyn std::fmt::Debug) {
        // don't care (yet?)
    }

    fn record_u64(&mut self, field: &tracing_core::Field, value: u64) {
        if field.name() == Self::TASK_ID_FIELD_NAME {
            self.id = Some(span::Id::from_u64(value));
        }
    }

    fn record_str(&mut self, field: &tracing_core::Field, value: &str) {
        if field.name() == "op" {
            self.op = Some(match value {
                Self::WAKE => WakeOp::Wake,
                Self::WAKE_BY_REF => WakeOp::WakeByRef,
                Self::CLONE => WakeOp::Clone,
                Self::DROP => WakeOp::Drop,
                _ => return,
            });
        }
    }
}

impl PollOpVisitor {
    pub(crate) const POLL_OP_EVENT_TARGET: &'static str = "runtime::resource::poll_op";
    const OP_NAME_FIELD_NAME: &'static str = "op_name";
    const OP_READINESS_FIELD_NAME: &'static str = "is_ready";

    pub(crate) fn result(self) -> Option<(String, bool)> {
        let op_name = self.op_name?;
        let is_ready = self.is_ready?;
        Some((op_name, is_ready))
    }
}

impl Visit for PollOpVisitor {
    fn record_debug(&mut self, _: &field::Field, _: &dyn std::fmt::Debug) {}

    fn record_bool(&mut self, field: &tracing_core::Field, value: bool) {
        if field.name() == Self::OP_READINESS_FIELD_NAME {
            self.is_ready = Some(value)
        }
    }

    fn record_str(&mut self, field: &tracing_core::Field, value: &str) {
        if field.name() == Self::OP_NAME_FIELD_NAME {
            self.op_name = Some(value.to_string());
        }
    }
}

impl StateUpdateVisitor {
    pub(crate) const STATE_UPDATE_EVENT_TARGET: &'static str = "runtime::resource::state_update";

    const STATE_OP_SUFFIX: &'static str = ".op";
    const STATE_UNIT_SUFFIX: &'static str = ".unit";

    const OP_ADD: &'static str = "add";
    const OP_SUB: &'static str = "sub";
    const OP_OVERRIDE: &'static str = "override";

    pub(crate) fn new(meta_id: proto::MetaId) -> Self {
        StateUpdateVisitor {
            meta_id,
            field: None,
            unit: None,
            op: None,
        }
    }

    pub(crate) fn result(self) -> Option<AttributeUpdate> {
        Some(AttributeUpdate {
            field: self.field?,
            op: self.op,
            unit: self.unit,
        })
    }
}

impl Visit for StateUpdateVisitor {
    fn record_debug(&mut self, field: &field::Field, value: &dyn std::fmt::Debug) {
        if !field.name().ends_with(Self::STATE_OP_SUFFIX)
            && !field.name().ends_with(Self::STATE_UNIT_SUFFIX)
        {
            self.field = Some(proto::Field {
                name: Some(field.name().into()),
                value: Some(value.into()),
                metadata_id: Some(self.meta_id.clone()),
            });
        }
    }

    fn record_i64(&mut self, field: &field::Field, value: i64) {
        if !field.name().ends_with(Self::STATE_OP_SUFFIX)
            && !field.name().ends_with(Self::STATE_UNIT_SUFFIX)
        {
            self.field = Some(proto::Field {
                name: Some(field.name().into()),
                value: Some(value.into()),
                metadata_id: Some(self.meta_id.clone()),
            });
        }
    }

    fn record_u64(&mut self, field: &field::Field, value: u64) {
        if !field.name().ends_with(Self::STATE_OP_SUFFIX)
            && !field.name().ends_with(Self::STATE_UNIT_SUFFIX)
        {
            self.field = Some(proto::Field {
                name: Some(field.name().into()),
                value: Some(value.into()),
                metadata_id: Some(self.meta_id.clone()),
            });
        }
    }

    fn record_bool(&mut self, field: &field::Field, value: bool) {
        if !field.name().ends_with(Self::STATE_OP_SUFFIX)
            && !field.name().ends_with(Self::STATE_UNIT_SUFFIX)
        {
            self.field = Some(proto::Field {
                name: Some(field.name().into()),
                value: Some(value.into()),
                metadata_id: Some(self.meta_id.clone()),
            });
        }
    }

    fn record_str(&mut self, field: &field::Field, value: &str) {
        if field.name().ends_with(Self::STATE_OP_SUFFIX) {
            match value {
                Self::OP_ADD => self.op = Some(AttributeUpdateOp::Add),
                Self::OP_SUB => self.op = Some(AttributeUpdateOp::Sub),
                Self::OP_OVERRIDE => self.op = Some(AttributeUpdateOp::Override),
                _ => {}
            };
        } else if field.name().ends_with(Self::STATE_UNIT_SUFFIX) {
            self.unit = Some(value.to_string());
        } else {
            self.field = Some(proto::Field {
                name: Some(field.name().into()),
                value: Some(value.into()),
                metadata_id: Some(self.meta_id.clone()),
            });
        }
    }
}
