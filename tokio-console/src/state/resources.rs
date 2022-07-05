use crate::intern::{self, InternedStr};
use crate::state::{
    format_location,
    id::{Id, Ids},
    Attribute, Field, Metadata, Visibility,
};
use crate::view;
use console_api as proto;
use std::{
    cell::RefCell,
    collections::HashMap,
    convert::{TryFrom, TryInto},
    rc::{Rc, Weak},
    time::{Duration, SystemTime},
};
use tui::{style::Color, text::Span};

#[derive(Default, Debug)]
pub(crate) struct ResourcesState {
    resources: HashMap<Id<Resource>, Rc<RefCell<Resource>>>,
    pub(crate) ids: Ids<Resource>,
    new_resources: Vec<ResourceRef>,
    dropped_events: u64,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum TypeVisibility {
    Public,
    Internal,
}

#[derive(Debug, Copy, Clone)]
#[repr(usize)]
pub(crate) enum SortBy {
    Rid = 0,
    Kind = 1,
    ConcreteType = 2,
    Target = 3,
    Total = 4,
}

#[derive(Debug)]
pub(crate) struct Resource {
    /// The resource's pretty (console-generated, sequential) ID.
    ///
    /// This is NOT the `tracing::span::Id` for the resource's `tracing` span on the
    /// remote.
    num: Id<Resource>,
    /// The `tracing::span::Id` on the remote process for this resource's span.
    ///
    /// This is used when requesting a resource details stream.
    span_id: u64,
    id_str: InternedStr,
    parent: InternedStr,
    parent_id: InternedStr,
    meta_id: u64,
    kind: InternedStr,
    stats: ResourceStats,
    target: InternedStr,
    concrete_type: InternedStr,
    location: String,
    visibility: TypeVisibility,
}

pub(crate) type ResourceRef = Weak<RefCell<Resource>>;

#[derive(Debug)]
struct ResourceStats {
    created_at: SystemTime,
    dropped_at: Option<SystemTime>,
    total: Option<Duration>,
    formatted_attributes: Vec<Vec<Span<'static>>>,
}

impl Default for SortBy {
    fn default() -> Self {
        Self::Rid
    }
}

impl SortBy {
    pub fn sort(&self, now: SystemTime, resources: &mut [Weak<RefCell<Resource>>]) {
        match self {
            Self::Rid => resources
                .sort_unstable_by_key(|resource| resource.upgrade().map(|r| r.borrow().num)),
            Self::Kind => resources.sort_unstable_by_key(|resource| {
                resource.upgrade().map(|r| r.borrow().kind.clone())
            }),
            Self::ConcreteType => resources.sort_unstable_by_key(|resource| {
                resource.upgrade().map(|r| r.borrow().concrete_type.clone())
            }),
            Self::Target => resources.sort_unstable_by_key(|resource| {
                resource.upgrade().map(|r| r.borrow().target.clone())
            }),
            Self::Total => resources
                .sort_unstable_by_key(|resource| resource.upgrade().map(|r| r.borrow().total(now))),
        }
    }
}

impl TryFrom<usize> for SortBy {
    type Error = ();
    fn try_from(idx: usize) -> Result<Self, Self::Error> {
        match idx {
            idx if idx == Self::Rid as usize => Ok(Self::Rid),
            idx if idx == Self::Kind as usize => Ok(Self::Kind),
            idx if idx == Self::ConcreteType as usize => Ok(Self::ConcreteType),
            idx if idx == Self::Target as usize => Ok(Self::Target),
            idx if idx == Self::Total as usize => Ok(Self::Total),
            _ => Err(()),
        }
    }
}

impl view::SortBy for SortBy {
    fn as_column(&self) -> usize {
        *self as usize
    }
}

impl ResourcesState {
    pub(crate) fn take_new_resources(&mut self) -> impl Iterator<Item = ResourceRef> + '_ {
        self.new_resources.drain(..)
    }

    pub(crate) fn resource(&self, id: Id<Resource>) -> Option<ResourceRef> {
        self.resources.get(&id).map(Rc::downgrade)
    }

    pub(crate) fn update_resources(
        &mut self,
        styles: &view::Styles,
        strings: &mut intern::Strings,
        metas: &HashMap<u64, Metadata>,
        update: proto::resources::ResourceUpdate,
        visibility: Visibility,
    ) {
        let parents: HashMap<Id<Resource>, ResourceRef> = update
            .new_resources
            .iter()
            .filter_map(|resource| {
                let span_id = resource.parent_resource_id?.id;
                let parent_id = self.ids.id_for(span_id);
                let parent = self.resource(parent_id)?;
                Some((parent_id, parent))
            })
            .collect();

        let mut stats_update = update.stats_update;
        let new_list = &mut self.new_resources;
        if matches!(visibility, Visibility::Show) {
            new_list.clear();
        }

        let new_resources = update.new_resources.into_iter().filter_map(|resource| {
            if resource.id.is_none() {
                tracing::warn!(?resource, "skipping resource with no id");
            }

            let meta_id = match resource.metadata.as_ref() {
                Some(id) => id.id,
                None => {
                    tracing::warn!(?resource, "resource has no metadata ID, skipping");
                    return None;
                }
            };
            let meta = match metas.get(&meta_id) {
                Some(meta) => meta,
                None => {
                    tracing::warn!(?resource, meta_id, "no metadata for resource, skipping");
                    return None;
                }
            };
            let kind = match kind_from_proto(resource.kind?, strings) {
                Ok(kind) => kind,
                Err(err) => {
                    tracing::warn!(%err, "resource kind cannot be parsed");
                    return None;
                }
            };

            let span_id = resource.id?.id;
            let stats =
                ResourceStats::from_proto(stats_update.remove(&span_id)?, meta, styles, strings);

            let num = self.ids.id_for(span_id);
            let parent_id = resource.parent_resource_id.map(|id| self.ids.id_for(id.id));

            let parent = strings.string(match parent_id {
                Some(id) => parents
                    .get(&id)
                    .and_then(|r| r.upgrade())
                    .map(|r| {
                        let r = r.borrow();
                        format!("{} ({}::{})", r.id(), r.target(), r.concrete_type())
                    })
                    .unwrap_or_else(|| id.to_string()),
                None => "n/a".to_string(),
            });

            let parent_id = strings.string(
                parent_id
                    .as_ref()
                    .map(Id::<Resource>::to_string)
                    .unwrap_or_else(|| "n/a".to_string()),
            );

            let location = format_location(resource.location);
            let visibility = if resource.is_internal {
                TypeVisibility::Internal
            } else {
                TypeVisibility::Public
            };

            let resource = Resource {
                num,
                span_id,
                id_str: strings.string(num.to_string()),
                parent,
                parent_id,
                kind,
                stats,
                target: meta.target.clone(),
                concrete_type: strings.string(resource.concrete_type),
                meta_id,
                location,
                visibility,
            };
            let resource = Rc::new(RefCell::new(resource));
            new_list.push(Rc::downgrade(&resource));
            Some((num, resource))
        });

        self.dropped_events += update.dropped_events;

        self.resources.extend(new_resources);

        for (span_id, stats) in stats_update {
            let num = self.ids.id_for(span_id);
            if let Some(resource) = self.resources.get_mut(&num) {
                let mut r = resource.borrow_mut();
                if let Some(meta) = metas.get(&r.meta_id) {
                    r.stats = ResourceStats::from_proto(stats, meta, styles, strings);
                }
            }
        }
    }

    pub(crate) fn retain_active(&mut self, now: SystemTime, retain_for: Duration) {
        self.resources.retain(|_, resource| {
            let resource = resource.borrow();

            resource
                .stats
                .dropped_at
                .map(|d| {
                    let dropped_for = now.duration_since(d).unwrap_or_default();
                    retain_for > dropped_for
                })
                .unwrap_or(true)
        })
    }

    pub(crate) fn dropped_events(&self) -> u64 {
        self.dropped_events
    }
}

impl Resource {
    pub(crate) fn id(&self) -> Id<Resource> {
        self.num
    }

    pub(crate) fn span_id(&self) -> u64 {
        self.span_id
    }

    pub(crate) fn id_str(&self) -> &str {
        &self.id_str
    }

    pub(crate) fn parent(&self) -> &str {
        &self.parent
    }

    pub(crate) fn parent_id(&self) -> &str {
        &self.parent_id
    }

    pub(crate) fn type_visibility(&self) -> TypeVisibility {
        self.visibility
    }

    pub(crate) fn target(&self) -> &str {
        &self.target
    }

    pub(crate) fn concrete_type(&self) -> &str {
        &self.concrete_type
    }

    pub(crate) fn kind(&self) -> &str {
        &self.kind
    }

    pub(crate) fn formatted_attributes(&self) -> &[Vec<Span<'static>>] {
        &self.stats.formatted_attributes
    }

    pub(crate) fn total(&self, since: SystemTime) -> Duration {
        self.stats.total.unwrap_or_else(|| {
            since
                .duration_since(self.stats.created_at)
                .unwrap_or_default()
        })
    }

    pub(crate) fn dropped(&self) -> bool {
        self.stats.total.is_some()
    }

    pub(crate) fn location(&self) -> &str {
        &self.location
    }
}

impl ResourceStats {
    fn from_proto(
        pb: proto::resources::Stats,
        meta: &Metadata,
        styles: &view::Styles,
        strings: &mut intern::Strings,
    ) -> Self {
        let mut pb = pb;
        let mut attributes = pb
            .attributes
            .drain(..)
            .filter_map(|pb| {
                let field = pb.field?;
                let field = Field::from_proto(field, meta, strings)?;
                Some(Attribute {
                    field,
                    unit: pb.unit,
                })
            })
            .collect::<Vec<_>>();

        let formatted_attributes = Attribute::make_formatted(styles, &mut attributes);
        let created_at = pb
            .created_at
            .expect("resource span was never created")
            .try_into()
            .unwrap();
        let dropped_at: Option<SystemTime> = pb.dropped_at.map(|v| v.try_into().unwrap());
        let total = dropped_at.map(|d| d.duration_since(created_at).unwrap_or_default());

        Self {
            created_at,
            dropped_at,
            total,
            formatted_attributes,
        }
    }
}

fn kind_from_proto(
    pb: proto::resources::resource::Kind,
    strings: &mut intern::Strings,
) -> Result<InternedStr, String> {
    use proto::resources::resource::kind::Kind::Known as PbKnown;
    use proto::resources::resource::kind::Kind::Other as PBOther;
    use proto::resources::resource::kind::Known::Timer as PbTimer;

    match pb.kind.expect("a resource should have a kind field") {
        PbKnown(known) if known == (PbTimer as i32) => Ok(strings.string("Timer".to_string())),
        PbKnown(known) => Err(format!("failed to parse known kind from {}", known)),
        PBOther(other) => Ok(strings.string(other)),
    }
}

impl TypeVisibility {
    pub(crate) fn render(self, styles: &crate::view::Styles) -> Span<'static> {
        const INT_UTF8: &str = "\u{1F512}";
        const PUB_UTF8: &str = "\u{2705}";
        match self {
            Self::Internal => Span::styled(styles.if_utf8(INT_UTF8, "INT"), styles.fg(Color::Red)),
            Self::Public => Span::styled(styles.if_utf8(PUB_UTF8, "PUB"), styles.fg(Color::Green)),
        }
    }
}
