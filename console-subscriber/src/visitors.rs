use super::{AttributeUpdate, AttributeUpdateOp, AttributeUpdateValue, Readiness, WakeOp};
use console_api as proto;
use std::collections::HashMap;
use tracing_core::{
    field::{self, Visit},
    span,
};

#[derive(Default)]
pub(crate) struct ResourceVisitor {
    concrete_type: Option<String>,
    kind: Option<String>,
}

pub(crate) struct FieldVisitor {
    fields: Vec<proto::Field>,
    meta_id: proto::MetaId,
}

#[derive(Default)]
pub(crate) struct AsyncOpVisitor {
    source: Option<String>,
}

#[derive(Default)]
pub(crate) struct WakerVisitor {
    id: Option<span::Id>,
    op: Option<WakeOp>,
}

#[derive(Default)]
pub(crate) struct ResourceOpVisitor {
    op_name: Option<String>,
    op_type: Option<String>,
    readiness: Option<Readiness>,
    state_text_attrs: HashMap<String, String>,
    state_numeric_attrs: HashMap<String, NumericStateAttr>,
}

#[derive(Debug)]
pub(crate) enum ResourceOpData {
    Poll {
        op_name: String,
        readiness: Readiness,
    },
    StateUpdate {
        op_name: String,
        attrs: Vec<AttributeUpdate>,
    },
}

#[derive(Default, Debug)]
struct NumericStateAttr {
    val: Option<u64>,
    op: Option<String>,
    unit: Option<String>,
}

impl ResourceVisitor {
    const RES_CONCRETE_TYPE_FIELD_NAME: &'static str = "concrete_type";
    const RES_KIND_FIELD_NAME: &'static str = "kind";

    pub(crate) fn result(self) -> Option<(String, String)> {
        self.concrete_type.zip(self.kind)
    }
}

impl Visit for ResourceVisitor {
    fn record_debug(&mut self, _: &field::Field, _: &dyn std::fmt::Debug) {}

    fn record_str(&mut self, field: &tracing_core::Field, value: &str) {
        if field.name() == Self::RES_CONCRETE_TYPE_FIELD_NAME {
            self.concrete_type = Some(value.to_string());
        } else if field.name() == Self::RES_KIND_FIELD_NAME {
            self.kind = Some(value.to_string());
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

impl ResourceOpVisitor {
    const OP_NAME_FIELD_NAME: &'static str = "op_name";
    const OP_TYPE_FIELD_NAME: &'static str = "op_type";
    const OP_READINESS_FIELD_NAME: &'static str = "readiness";

    const OP_TYPE_STATE_UPDATE: &'static str = "state_update";
    const OP_TYPE_POLL: &'static str = "poll";
    const OP_READINESS_READY: &'static str = "ready";
    const OP_READINESS_PENDING: &'static str = "pending";

    const OP_STATE_FIELD_PREFIX: &'static str = "state";
    const OP_STATE_FIELD_TYPE_UNIT: &'static str = "unit";
    const OP_STATE_FIELD_TYPE_VALUE: &'static str = "value";
    const OP_STATE_FIELD_TYPE_OP: &'static str = "op";

    const UPDATE_OP_ADD: &'static str = "add";
    const UPDATE_OP_SUB: &'static str = "sub";
    const UPDATE_OP_OVR: &'static str = "ovr";

    pub(crate) fn result(self) -> Option<ResourceOpData> {
        if let Some(op_name) = self.op_name {
            if let Some(op_type) = self.op_type {
                if op_type == Self::OP_TYPE_POLL {
                    if let Some(readiness) = self.readiness {
                        return Some(ResourceOpData::Poll { op_name, readiness });
                    }
                    return None;
                } else if op_type == Self::OP_TYPE_STATE_UPDATE {
                    let numeric_updates =
                        self.state_numeric_attrs
                            .into_iter()
                            .filter_map(|(attr_name, attr)| {
                                if let NumericStateAttr {
                                    val: Some(val),
                                    op: Some(op),
                                    unit: Some(unit),
                                } = attr
                                {
                                    let op = match op.as_str() {
                                        Self::UPDATE_OP_ADD => Some(AttributeUpdateOp::Add),
                                        Self::UPDATE_OP_SUB => Some(AttributeUpdateOp::Sub),
                                        Self::UPDATE_OP_OVR => Some(AttributeUpdateOp::Ovr),
                                        _ => None,
                                    };

                                    return op.map(|op| AttributeUpdate {
                                        name: attr_name,
                                        val: AttributeUpdateValue::Numeric { val, op, unit },
                                    });
                                }
                                None
                            });

                    let text_updates =
                        self.state_text_attrs.into_iter().map(|(attr_name, attr)| {
                            AttributeUpdate {
                                name: attr_name,
                                val: AttributeUpdateValue::Text(attr),
                            }
                        });

                    let attrs = numeric_updates.chain(text_updates).collect();
                    return Some(ResourceOpData::StateUpdate { op_name, attrs });
                }
            }
        }
        None
    }
}

struct MatchPart {
    attr_name: String,
    part_type: MatchPartType,
}

enum MatchPartType {
    OpType(String),
    Unit(String),
    Text(String),
}

impl Visit for ResourceOpVisitor {
    fn record_debug(&mut self, _: &field::Field, _: &dyn std::fmt::Debug) {}

    fn record_str(&mut self, field: &tracing_core::Field, value: &str) {
        let extract_partial_attr = || -> Option<MatchPart> {
            let parts: Vec<_> = field.name().split('_').collect();
            if parts.len() == 3 {
                let p0 = parts[0];
                let p1 = parts[1];
                let p2 = parts[2];
                if p0 == Self::OP_STATE_FIELD_PREFIX {
                    if p2 == Self::OP_STATE_FIELD_TYPE_UNIT {
                        return Some(MatchPart {
                            attr_name: p1.into(),
                            part_type: MatchPartType::Unit(value.into()),
                        });
                    } else if p2 == Self::OP_STATE_FIELD_TYPE_VALUE {
                        return Some(MatchPart {
                            attr_name: p1.into(),
                            part_type: MatchPartType::Text(value.into()),
                        });
                    } else if p2 == Self::OP_STATE_FIELD_TYPE_OP {
                        return Some(MatchPart {
                            attr_name: p1.into(),
                            part_type: MatchPartType::OpType(value.into()),
                        });
                    }
                }
            }
            None
        };

        if field.name() == Self::OP_NAME_FIELD_NAME {
            self.op_name = Some(value.to_string());
            return;
        } else if field.name() == Self::OP_TYPE_FIELD_NAME {
            self.op_type = Some(value.to_string());
            return;
        } else if field.name() == Self::OP_READINESS_FIELD_NAME {
            self.readiness = Some(match value {
                Self::OP_READINESS_READY => Readiness::Ready,
                Self::OP_READINESS_PENDING => Readiness::Pending,
                _ => return,
            });
            return;
        }

        if let Some(match_part) = extract_partial_attr() {
            match match_part.part_type {
                MatchPartType::Text(t) => {
                    self.state_text_attrs.insert(match_part.attr_name, t);
                }
                MatchPartType::OpType(op_type) => {
                    let attr = self
                        .state_numeric_attrs
                        .entry(match_part.attr_name)
                        .or_default();
                    attr.op = Some(op_type);
                }
                MatchPartType::Unit(unit) => {
                    let attr = self
                        .state_numeric_attrs
                        .entry(match_part.attr_name)
                        .or_default();
                    attr.unit = Some(unit);
                }
            }
        }
    }

    fn record_u64(&mut self, field: &tracing_core::Field, value: u64) {
        let parts: Vec<_> = field.name().split('_').collect();
        if parts.len() == 3 {
            let p0 = parts[0];
            let p1 = parts[1];
            let p2 = parts[2];
            if p0 == Self::OP_STATE_FIELD_PREFIX && p2 == Self::OP_STATE_FIELD_TYPE_VALUE {
                let attr = self.state_numeric_attrs.entry(p1.into()).or_default();
                attr.val = Some(value)
            }
        }
    }
}
