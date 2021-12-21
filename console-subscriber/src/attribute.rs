use console_api as proto;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub(crate) struct Attributes {
    attributes: HashMap<FieldKey, proto::Attribute>,
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub(crate) struct FieldKey {
    update_id: u64,
    field_name: proto::field::Name,
}

// === impl Attributes ===

impl Attributes {
    pub(crate) fn values(&self) -> impl Iterator<Item = &proto::Attribute> {
        self.attributes.values()
    }
}
