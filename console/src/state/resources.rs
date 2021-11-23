use crate::intern::{self, InternedStr};
use crate::state::{format_location, Attribute, Field, Metadata, Visibility};
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
    resources: HashMap<u64, Rc<RefCell<Resource>>>,
    new_resources: Vec<ResourceRef>,
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

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum Kind {
    Timer,
    Other(InternedStr),
}

#[derive(Debug)]
pub(crate) struct Resource {
    id: u64,
    parent_id: Option<u64>,
    meta_id: u64,
    kind: Kind,
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
    pub fn sort(&self, now: SystemTime, resources: &mut Vec<Weak<RefCell<Resource>>>) {
        match self {
            Self::Rid => {
                resources.sort_unstable_by_key(|resource| resource.upgrade().map(|r| r.borrow().id))
            }
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

    pub(crate) fn resource(&self, id: u64) -> Option<ResourceRef> {
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
            let kind = match Kind::from_proto(resource.kind?, strings) {
                Ok(kind) => kind,
                Err(err) => {
                    tracing::warn!(%err, "resource kind cannot be parsed");
                    return None;
                }
            };

            let id = resource.id?.id;
            let parent_id = resource.parent_resource_id.map(|id| id.id);
            let stats = ResourceStats::from_proto(stats_update.remove(&id)?, meta, styles, strings);
            let location = format_location(resource.location);
            let visibility = if resource.is_internal {
                TypeVisibility::Internal
            } else {
                TypeVisibility::Public
            };

            let resource = Resource {
                id,
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
            Some((id, resource))
        });
        self.resources.extend(new_resources);

        for (id, stats) in stats_update {
            if let Some(resource) = self.resources.get_mut(&id) {
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
                    let dropped_for = now.duration_since(d).unwrap();
                    retain_for > dropped_for
                })
                .unwrap_or(true)
        })
    }
}

impl Resource {
    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    pub(crate) fn parent_id(&self) -> Option<u64> {
        self.parent_id
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
        match &self.kind {
            Kind::Timer => "Timer",
            Kind::Other(other) => other,
        }
    }

    pub(crate) fn formatted_attributes(&self) -> &[Vec<Span<'static>>] {
        &self.stats.formatted_attributes
    }

    pub(crate) fn total(&self, since: SystemTime) -> Duration {
        self.stats
            .total
            .unwrap_or_else(|| since.duration_since(self.stats.created_at).unwrap())
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
        let total = dropped_at.map(|d| d.duration_since(created_at).unwrap());

        Self {
            created_at,
            dropped_at,
            total,
            formatted_attributes,
        }
    }
}

impl Kind {
    fn from_proto(
        pb: proto::resources::resource::Kind,
        strings: &mut intern::Strings,
    ) -> Result<Self, String> {
        use proto::resources::resource::kind::Kind::Known as PbKnown;
        use proto::resources::resource::kind::Kind::Other as PBOther;
        use proto::resources::resource::kind::Known::Timer as PbTimer;

        match pb.kind.expect("a resource should have a kind field") {
            PbKnown(known) if known == (PbTimer as i32) => Ok(Kind::Timer),
            PbKnown(known) => Err(format!("failed to parse known kind from {}", known)),
            PBOther(other) => Ok(Kind::Other(strings.string(other))),
        }
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
