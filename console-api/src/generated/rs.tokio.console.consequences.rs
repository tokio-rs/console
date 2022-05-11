#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Causality {
    #[prost(oneof="causality::Update", tags="1, 2, 3, 4, 5, 6")]
    pub update: ::core::option::Option<causality::Update>,
}
/// Nested message and enum types in `Causality`.
pub mod causality {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Update {
        #[prost(message, tag="1")]
        Extant(super::Extant),
        #[prost(message, tag="2")]
        OpenDirect(super::OpenDirect),
        #[prost(message, tag="3")]
        NewIndirect(super::NewIndirect),
        #[prost(message, tag="4")]
        CloseDirect(super::CloseDirect),
        #[prost(message, tag="5")]
        CloseIndirect(super::CloseIndirect),
        #[prost(message, tag="6")]
        CloseCyclic(super::CloseCyclic),
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Span {
    #[prost(message, optional, tag="1")]
    pub span_id: ::core::option::Option<super::common::SpanId>,
    #[prost(message, optional, tag="2")]
    pub metadata_id: ::core::option::Option<super::common::MetaId>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Extant {
    #[prost(message, optional, tag="1")]
    pub cause: ::core::option::Option<Span>,
    #[prost(message, repeated, tag="2")]
    pub direct_consequences: ::prost::alloc::vec::Vec<Span>,
    #[prost(message, repeated, tag="3")]
    pub indirect_consequences: ::prost::alloc::vec::Vec<Span>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OpenDirect {
    #[prost(message, optional, tag="1")]
    pub cause: ::core::option::Option<Span>,
    #[prost(message, optional, tag="2")]
    pub direct_consequences: ::core::option::Option<Span>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NewIndirect {
    #[prost(message, optional, tag="1")]
    pub cause: ::core::option::Option<Span>,
    #[prost(message, optional, tag="3")]
    pub indirect_consequences: ::core::option::Option<Span>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CloseDirect {
    #[prost(message, optional, tag="1")]
    pub span: ::core::option::Option<Span>,
    #[prost(message, optional, tag="2")]
    pub direct_cause: ::core::option::Option<Span>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CloseIndirect {
    #[prost(message, optional, tag="1")]
    pub span: ::core::option::Option<Span>,
    #[prost(message, repeated, tag="3")]
    pub indirect_causes: ::prost::alloc::vec::Vec<Span>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CloseCyclic {
    #[prost(message, optional, tag="1")]
    pub span: ::core::option::Option<Span>,
    #[prost(message, optional, tag="2")]
    pub direct_cause: ::core::option::Option<Span>,
    #[prost(message, repeated, tag="3")]
    pub indirect_causes: ::prost::alloc::vec::Vec<Span>,
}
