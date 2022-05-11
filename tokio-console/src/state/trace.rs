use console_api as proto;
use tracing_causality::{Span, Update};

fn span_from_pb(proto: proto::consequences::Span) -> Span<u64> {
    Span {
        id: proto.span_id.unwrap().into(),
        metadata: proto.metadata_id.unwrap().id,
    }
}

pub(crate) fn updates_from_pb(
    proto: proto::consequences::causality::Update,
) -> Box<dyn Iterator<Item = Update<u64>>> {
    use proto::consequences::causality::Update as ProtoUpdate;
    use std::iter::{once, repeat};
    match proto {
        ProtoUpdate::Extant(proto::consequences::Extant {
            cause: Some(cause),
            direct_consequences,
            indirect_consequences,
        }) => {
            let cause = span_from_pb(cause);
            let direct_consequences = direct_consequences
                .into_iter()
                .zip(repeat(cause.clone()))
                .map(
                    move |(consequence, cause)| tracing_causality::Update::OpenDirect {
                        cause: cause,
                        consequence: span_from_pb(consequence),
                    },
                );
            let indirect_consequences = indirect_consequences.into_iter().zip(repeat(cause)).map(
                move |(consequence, cause)| tracing_causality::Update::NewIndirect {
                    cause: cause,
                    consequence: span_from_pb(consequence),
                },
            );
            Box::new(direct_consequences.chain(indirect_consequences))
        }
        ProtoUpdate::OpenDirect(proto::consequences::OpenDirect {
            cause: Some(cause),
            direct_consequences: Some(consequence),
        }) => Box::new(once(tracing_causality::Update::OpenDirect {
            cause: span_from_pb(cause),
            consequence: span_from_pb(consequence),
        })),
        ProtoUpdate::NewIndirect(proto::consequences::NewIndirect {
            cause: Some(cause),
            indirect_consequences: Some(consequence),
        }) => Box::new(once(tracing_causality::Update::NewIndirect {
            cause: span_from_pb(cause),
            consequence: span_from_pb(consequence),
        })),
        ProtoUpdate::CloseDirect(proto::consequences::CloseDirect {
            span: Some(span),
            direct_cause,
        }) => Box::new(once(tracing_causality::Update::CloseDirect {
            span: span_from_pb(span),
            direct_cause: direct_cause.map(span_from_pb),
        })),
        ProtoUpdate::CloseIndirect(proto::consequences::CloseIndirect {
            span: Some(span),
            indirect_causes,
        }) => Box::new(once(tracing_causality::Update::CloseIndirect {
            span: span_from_pb(span),
            indirect_causes: indirect_causes.into_iter().map(span_from_pb).collect(),
        })),
        ProtoUpdate::CloseCyclic(proto::consequences::CloseCyclic {
            span: Some(span),
            direct_cause,
            indirect_causes,
        }) => Box::new(once(tracing_causality::Update::CloseCyclic {
            span: span_from_pb(span),
            direct_cause: direct_cause.map(span_from_pb),
            indirect_causes: indirect_causes.into_iter().map(span_from_pb).collect(),
        })),
        _ => unimplemented!(),
    }
}
