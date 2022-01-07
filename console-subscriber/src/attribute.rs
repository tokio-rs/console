use console_api as proto;
use std::collections::HashMap;
use tracing::span::Id;

#[derive(Debug, Default)]
pub(crate) struct Attributes {
    attributes: HashMap<FieldKey, proto::Attribute>,
}

#[derive(Debug, Clone)]
pub(crate) struct Update {
    pub(crate) field: proto::Field,
    pub(crate) op: Option<UpdateOp>,
    pub(crate) unit: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) enum UpdateOp {
    Add,
    Override,
    Sub,
}

/// Represents a key for a `proto::field::Name`. Because the
/// proto::field::Name might not be unique we also include the
/// resource id in this key
#[derive(Debug, Hash, PartialEq, Eq)]
struct FieldKey {
    update_id: Id,
    field_name: proto::field::Name,
}

// === impl Attributes ===

impl Attributes {
    pub(crate) fn values(&self) -> impl Iterator<Item = &proto::Attribute> {
        self.attributes.values()
    }

    pub(crate) fn update(&mut self, id: &Id, update: &Update) {
        let field_name = match update.field.name.as_ref() {
            Some(name) => name.clone(),
            None => {
                tracing::warn!(?update.field, "field missing name, skipping...");
                return;
            }
        };
        let update_id = id.clone();
        let key = FieldKey {
            update_id,
            field_name,
        };

        self.attributes
            .entry(key)
            .and_modify(|attr| update_attribute(attr, &update))
            .or_insert_with(|| update.clone().into());
    }
}

fn update_attribute(attribute: &mut proto::Attribute, update: &Update) {
    use proto::field::Value::*;
    let attribute_val = attribute.field.as_mut().and_then(|a| a.value.as_mut());
    let update_val = update.field.value.clone();
    let update_name = update.field.name.clone();
    match (attribute_val, update_val) {
        (Some(BoolVal(v)), Some(BoolVal(upd))) => *v = upd,

        (Some(StrVal(v)), Some(StrVal(upd))) => *v = upd,

        (Some(DebugVal(v)), Some(DebugVal(upd))) => *v = upd,

        (Some(U64Val(v)), Some(U64Val(upd))) => match update.op {
            Some(UpdateOp::Add) => *v = v.saturating_add(upd),

            Some(UpdateOp::Sub) => *v = v.saturating_sub(upd),

            Some(UpdateOp::Override) => *v = upd,

            None => tracing::warn!(
                "numeric attribute update {:?} needs to have an op field",
                update_name
            ),
        },

        (Some(I64Val(v)), Some(I64Val(upd))) => match update.op {
            Some(UpdateOp::Add) => *v = v.saturating_add(upd),

            Some(UpdateOp::Sub) => *v = v.saturating_sub(upd),

            Some(UpdateOp::Override) => *v = upd,

            None => tracing::warn!(
                "numeric attribute update {:?} needs to have an op field",
                update_name
            ),
        },

        (val, update) => {
            tracing::warn!(
                "attribute {:?} cannot be updated by update {:?}",
                val,
                update
            );
        }
    }
}

impl From<Update> for proto::Attribute {
    fn from(upd: Update) -> Self {
        proto::Attribute {
            field: Some(upd.field),
            unit: upd.unit,
        }
    }
}
