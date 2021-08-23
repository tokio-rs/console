//! These visitors are respondible for extracing the relevan
//! fields from tracing metadata and producing the parts
//! needed to construct `Event` instances.

use super::{AttributeUpdate, AttributeUpdateOp, Readiness, WakeOp};
use console_api as proto;
use proto::field::Name as PbFieldName;
use proto::field::Value as PbFieldValue;
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
///     target: "tokio::resource::poll_op",
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
    readiness: Option<Readiness>,
}

/// Used to extract the fields needed to construct
/// an Event::StateUpdate from the metadata of a tracing event
/// that has the following shape:
///
/// tracing::trace!(
///     target: "tokio::resource::state_update",
///     duration = "attribute_name",
///     value = 10,
///     unit = "ms",
///     op = "ovr",
/// );
///
/// Fields:
/// attribute_name - a field value for a field that has the name of the resource attribute being updated
/// value - the value for this update
/// unit - the unit for the value being updated (e.g. ms, s, bytes)
/// op - the operation that this update performs to the value of the resource attribute (one of: ovr, sub, add)
#[derive(Default)]
pub(crate) struct StateUpdateVisitor {
    name: Option<String>,
    val: Option<PbFieldValue>,
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
    pub(crate) const POLL_OP_EVENT_NAME: &'static str = "runtime.resource.poll_op";
    const OP_NAME_FIELD_NAME: &'static str = "op_name";
    const OP_READINESS_FIELD_NAME: &'static str = "readiness";
    const OP_READINESS_READY: &'static str = "ready";
    const OP_READINESS_PENDING: &'static str = "pending";

    pub(crate) fn result(self) -> Option<(String, Readiness)> {
        let op_name = self.op_name?;
        let readiness = self.readiness?;
        Some((op_name, readiness))
    }
}

impl Visit for PollOpVisitor {
    fn record_debug(&mut self, _: &field::Field, _: &dyn std::fmt::Debug) {}

    fn record_str(&mut self, field: &tracing_core::Field, value: &str) {
        match field.name() {
            Self::OP_NAME_FIELD_NAME => {
                self.op_name = Some(value.to_string());
            }
            Self::OP_READINESS_FIELD_NAME => {
                self.readiness = Some(match value {
                    Self::OP_READINESS_READY => Readiness::Ready,
                    Self::OP_READINESS_PENDING => Readiness::Pending,
                    _ => return,
                });
            }
            _ => {}
        }
    }
}

impl StateUpdateVisitor {
    pub(crate) const STATE_UPDATE_EVENT_NAME: &'static str = "runtime.resource.state_update";
    const OP_STATE_FIELD_TYPE_ATTR_NAME: &'static str = "attribute_name";
    const OP_STATE_FIELD_TYPE_VALUE: &'static str = "value";
    const OP_STATE_FIELD_TYPE_UNIT: &'static str = "unit";
    const OP_STATE_FIELD_TYPE_OP: &'static str = "op";
    const UPDATE_OP_ADD: &'static str = "add";
    const UPDATE_OP_SUB: &'static str = "sub";
    const UPDATE_OP_OVR: &'static str = "ovr";

    pub(crate) fn result(self, metadata_id: proto::MetaId) -> Option<AttributeUpdate> {
        let name = self.name?;
        let value = self.val?;
        let op = self.op?;

        let val = proto::Field {
            metadata_id: Some(metadata_id),
            name: Some(PbFieldName::StrName(name)),
            value: Some(value),
        };

        Some(AttributeUpdate {
            val,
            op,
            unit: self.unit,
        })
    }
}

impl Visit for StateUpdateVisitor {
    fn record_debug(&mut self, _: &field::Field, _: &dyn std::fmt::Debug) {}

    fn record_i64(&mut self, field: &field::Field, value: i64) {
        if field.name() == Self::OP_STATE_FIELD_TYPE_VALUE {
            self.val = Some(PbFieldValue::I64Val(value));
        }
    }

    fn record_u64(&mut self, field: &field::Field, value: u64) {
        if field.name() == Self::OP_STATE_FIELD_TYPE_VALUE {
            self.val = Some(PbFieldValue::U64Val(value));
        }
    }

    fn record_bool(&mut self, field: &field::Field, value: bool) {
        if field.name() == Self::OP_STATE_FIELD_TYPE_VALUE {
            self.val = Some(PbFieldValue::BoolVal(value));
        }
    }

    fn record_str(&mut self, field: &field::Field, value: &str) {
        if value == Self::OP_STATE_FIELD_TYPE_ATTR_NAME {
            self.name = Some(field.name().to_string());
            return;
        }

        match field.name() {
            Self::OP_STATE_FIELD_TYPE_UNIT => self.unit = Some(value.to_string()),

            Self::OP_STATE_FIELD_TYPE_OP => {
                match value {
                    Self::UPDATE_OP_ADD => self.op = Some(AttributeUpdateOp::Add),
                    Self::UPDATE_OP_SUB => self.op = Some(AttributeUpdateOp::Sub),
                    Self::UPDATE_OP_OVR => self.op = Some(AttributeUpdateOp::Ovr),
                    _ => {}
                };
            }

            Self::OP_STATE_FIELD_TYPE_VALUE => {
                self.val = Some(PbFieldValue::StrVal(value.to_string()));
            }

            _ => {}
        }
    }
}
