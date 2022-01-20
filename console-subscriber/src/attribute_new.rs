use crate::ToProto;
use console_api as proto;
use proto::field::Value as UpdateValue;
use proto::{field::Name, MetaId};
use std::collections::HashMap;
use std::ptr;
use std::sync::atomic::{AtomicPtr, AtomicU64, AtomicU8, Ordering::*};
use tracing::field::FieldSet;

#[derive(Debug)]
pub(crate) struct Attributes {
    attributes: HashMap<proto::field::Name, Attribute>,
}

#[derive(Debug)]
pub(crate) struct Attribute {
    name: proto::field::Name,
    meta_id: MetaId,
    value: Value,
    unit: Option<String>,
}

#[derive(Debug)]
pub(crate) struct Value {
    str_val: AtomicPtr<String>,
    other_val: AtomicU64,
    val_type: AtomicU8,
}

const EMPTY: u8 = 0;
const BOOL: u8 = 1;
const U64: u8 = 2;
const I64: u8 = 3;
const STR: u8 = 4;
const DEBUG: u8 = 5;

#[derive(Debug, Clone)]
pub(crate) struct Update {
    name: proto::field::Name,
    is_delta: bool,
    value: proto::field::Value,
}

// // === impl Attributes ===

impl Attributes {
    pub(crate) const STATE_PREFIX: &'static str = "state.";

    pub(crate) fn new(meta_id: MetaId, fields: &FieldSet) -> Self {
        let attributes = fields
            .iter()
            .filter_map(|field| {
                if field.name().starts_with(Attributes::STATE_PREFIX) {
                    let mut parts = field.name().split('.');
                    parts.next();
                    if let Some(name) = parts.next() {
                        return Some((name.into(), parts.next()));
                    }
                }
                None
            })
            .map(|(name, unit): (Name, Option<&str>)| {
                let value = Value {
                    str_val: AtomicPtr::new(ptr::null_mut()),
                    other_val: AtomicU64::new(0),
                    val_type: AtomicU8::new(0),
                };
                let unit = unit.map(Into::into);

                let attr = Attribute {
                    name: name.clone(),
                    meta_id: meta_id.clone(),
                    unit,
                    value,
                };
                (name, attr)
            })
            .collect();

        Self { attributes }
    }

    pub(crate) fn update(&self, update: &Update) {
        if let Some(attr) = self.attributes.get(&update.name) {
            let is_delta = update.is_delta;
            let perv_type = attr.value.val_type.swap(update.update_type(), AcqRel);
            match (perv_type, &update.value) {
                (BOOL | EMPTY, UpdateValue::BoolVal(upd)) => {
                    attr.value.other_val.store(*upd as u64, Release);
                }

                (STR, UpdateValue::StrVal(upd)) => {
                    attr.value
                        .str_val
                        .store(Box::into_raw(Box::new(upd.clone())), Release);
                }

                (DEBUG, UpdateValue::DebugVal(upd)) => {
                    attr.value
                        .str_val
                        .store(Box::into_raw(Box::new(upd.clone())), Release);
                }

                (U64 | EMPTY, UpdateValue::U64Val(upd)) => {
                    if is_delta && perv_type != EMPTY {
                        attr.value.other_val.fetch_add(*upd, Release);
                    } else {
                        attr.value.other_val.store(*upd, Release);
                    }
                }
                (I64 | EMPTY, UpdateValue::I64Val(upd)) => {
                    if is_delta && perv_type != EMPTY {
                        attr.value
                            .other_val
                            .fetch_update(AcqRel, Acquire, |v| {
                                Some(((v as i64) + (*upd as i64)) as u64)
                            })
                            .unwrap();
                    } else {
                        attr.value.other_val.store(*upd as u64, Release);
                    }
                }
                (val, update) => {
                    tracing::warn!(
                        "attribute {:?} cannot be updated by update {:?}",
                        val,
                        update
                    );
                }
            }
        }
    }

    pub(crate) fn values(&self) -> impl Iterator<Item = &Attribute> {
        self.attributes.values()
    }
}

// // === impl Update ===

impl Update {
    pub(crate) fn new(
        name: proto::field::Name,
        value: proto::field::Value,
        is_delta: bool,
    ) -> Self {
        Self {
            name,
            is_delta,
            value,
        }
    }
    fn update_type(&self) -> u8 {
        match self.value {
            UpdateValue::BoolVal(_) => BOOL,
            UpdateValue::StrVal(_) => STR,
            UpdateValue::DebugVal(_) => DEBUG,
            UpdateValue::U64Val(_) => U64,
            UpdateValue::I64Val(_) => I64,
        }
    }
}

impl ToProto for Attribute {
    type Output = Option<proto::Attribute>;

    fn to_proto(&self) -> Self::Output {
        if let Some(value) = self.value.to_proto() {
            return Some(proto::Attribute {
                field: Some(proto::Field {
                    metadata_id: Some(self.meta_id.clone()),
                    name: Some(self.name.clone()),
                    value: Some(value),
                }),
                unit: self.unit.clone(),
            });
        }
        None
    }
}

impl ToProto for Value {
    type Output = Option<proto::field::Value>;

    fn to_proto(&self) -> Self::Output {
        use proto::field::Value as ProtoVal;
        match self.val_type.load(Acquire) {
            BOOL => Some(ProtoVal::BoolVal(self.other_val.load(Acquire) != 0)),
            U64 => Some(ProtoVal::U64Val(self.other_val.load(Acquire) as u64)),
            I64 => Some(ProtoVal::I64Val(self.other_val.load(Acquire) as i64)),
            DEBUG => Some(ProtoVal::StrVal("HAHA".to_string())),
            STR => Some(ProtoVal::StrVal("HAHA".to_string())),
            _ => None,
        }
    }
}
