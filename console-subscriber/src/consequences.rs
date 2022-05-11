use console_api as proto;
use std::fmt;
use tracing::{Dispatch, Subscriber};
use tracing_causality as causality;
use tracing_subscriber::registry::LookupSpan;

pub(crate) struct Tracer {
    dispatch: Dispatch,
    trace: fn(&Dispatch, &tracing::Id, usize) -> Option<(causality::Trace, causality::Updates)>,
}

impl fmt::Debug for Tracer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tracer")
            .field("dispatch", &self.dispatch)
            .field("trace", &"fn(){}")
            .finish()
    }
}

impl Tracer {
    pub(crate) fn from_dispatch<S>(dispatch: &tracing::Dispatch) -> Self
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        fn trace<S>(
            dispatch: &tracing::Dispatch,
            id: &tracing::Id,
            capacity: usize,
        ) -> Option<(causality::Trace, causality::Updates)>
        where
            S: Subscriber + for<'a> LookupSpan<'a>,
        {
            causality::trace(dispatch.downcast_ref::<S>()?, id, capacity)
        }

        let _ = dispatch
            .downcast_ref::<S>()
            .expect("subscriber should downcast to expected type; this is a bug!");

        Self {
            dispatch: dispatch.clone(),
            trace: trace::<S>,
        }
    }

    pub(crate) fn trace(
        &self,
        id: &tracing::Id,
        update_capacity: usize,
    ) -> Option<(causality::Trace, causality::Updates)> {
        (self.trace)(&self.dispatch, id, update_capacity)
    }
}

fn into_proto_span(span: causality::Span) -> proto::consequences::Span {
    proto::consequences::Span {
        span_id: Some(span.id.into()),
        metadata_id: Some(span.metadata.into()),
    }
}

pub(crate) fn trace_into_proto(trace: causality::Trace) -> Vec<proto::consequences::Causality> {
    trace
        .iter()
        .map(|(cause, consequences)| {
            let update =
                proto::consequences::causality::Update::Extant(proto::consequences::Extant {
                    cause: Some(into_proto_span(cause)),
                    direct_consequences: consequences.iter_direct().map(into_proto_span).collect(),
                    indirect_consequences: consequences
                        .iter_indirect()
                        .map(into_proto_span)
                        .collect(),
                });
            proto::consequences::Causality {
                update: Some(update),
            }
        })
        .collect()
}

pub(crate) fn updates_into_proto(
    updates: &causality::Updates,
) -> Vec<proto::consequences::Causality> {
    updates
        .drain()
        .map(|update| {
            let update = match update {
                causality::Update::OpenDirect { cause, consequence } => {
                    proto::consequences::causality::Update::OpenDirect(
                        proto::consequences::OpenDirect {
                            cause: Some(into_proto_span(cause)),
                            direct_consequences: Some(into_proto_span(consequence)),
                        },
                    )
                }
                causality::Update::NewIndirect { cause, consequence } => {
                    proto::consequences::causality::Update::NewIndirect(
                        proto::consequences::NewIndirect {
                            cause: Some(into_proto_span(cause)),
                            indirect_consequences: Some(into_proto_span(consequence)),
                        },
                    )
                }
                causality::Update::CloseDirect { span, direct_cause } => {
                    proto::consequences::causality::Update::CloseDirect(
                        proto::consequences::CloseDirect {
                            span: Some(into_proto_span(span)),
                            direct_cause: direct_cause.map(into_proto_span),
                        },
                    )
                }
                causality::Update::CloseIndirect {
                    span,
                    indirect_causes,
                } => proto::consequences::causality::Update::CloseIndirect(
                    proto::consequences::CloseIndirect {
                        span: Some(into_proto_span(span)),
                        indirect_causes: indirect_causes.into_iter().map(into_proto_span).collect(),
                    },
                ),
                causality::Update::CloseCyclic {
                    span,
                    direct_cause,
                    indirect_causes,
                } => proto::consequences::causality::Update::CloseCyclic(
                    proto::consequences::CloseCyclic {
                        span: Some(into_proto_span(span)),
                        direct_cause: direct_cause.map(into_proto_span),
                        indirect_causes: indirect_causes.into_iter().map(into_proto_span).collect(),
                    },
                ),
            };
            proto::consequences::Causality {
                update: Some(update),
            }
        })
        .collect()
}
