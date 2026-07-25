//! `subscriptions/listen` wire types (MCP 2026-07-28, HTTP-04).
//!
//! The 2026-07-28 schema **removes** `resources/subscribe` and
//! `resources/unsubscribe` entirely and replaces them with a single long-lived
//! notification stream opened by a `subscriptions/listen` request. The only
//! surviving mention of the retired RPC is the doc comment on
//! [`SubscriptionFilter::resource_subscriptions`].
//!
//! # The stream protocol these types serve
//!
//! 1. The client sends `subscriptions/listen` with a [`SubscriptionFilter`] under
//!    the REQUIRED `notifications` field.
//! 2. The server answers with an SSE stream whose FIRST frame is a
//!    [`ACKNOWLEDGED_METHOD`] notification carrying
//!    [`SubscriptionAcknowledgedParams`] — its `notifications` field is the
//!    subset the server agreed to honor, never a superset of what was requested.
//! 3. Every frame carries `_meta[`[`SUBSCRIPTION_ID_META_KEY`]`]`, equal to the
//!    JSON-RPC id of the listen request.
//! 4. On graceful teardown the server sends [`SubscriptionsListenResult`] as the
//!    JSON-RPC response before closing.
//!
//! # Field types are LOCKED to the checkpoint record, not to prose
//!
//! Every Rust field type below is taken from `113-SPEC-RECHECK.md` § A.6, which
//! transcribed `schema/draft/schema.ts` @ `71e306956a4959c9655e5036be215d41986596e6`
//! verbatim. `resourceSubscriptions` in particular is `string[]` — an array of
//! resource URIs — and NOT a boolean and NOT a map. Guessing that shape is how a
//! silent interop failure ships, so
//! [`tests::filter_matches_the_shape_recorded_in_the_spec_recheck`] deserializes
//! the recorded declaration and
//! [`tests::a_boolean_resource_subscriptions_is_rejected`] pins the negative.

use crate::types::capabilities::ServerCapabilities;
use crate::types::jsonrpc::RequestId;
use crate::types::notifications::ServerNotification;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The `subscriptions/listen` JSON-RPC method name.
///
/// A named constant rather than a scattered literal, matching the
/// `RESERVED_*_KEY` constant style in [`crate::types::protocol::context`].
pub const SUBSCRIPTIONS_LISTEN_METHOD: &str = "subscriptions/listen";

/// The method of the acknowledgement notification a listen stream MUST send
/// first.
pub const ACKNOWLEDGED_METHOD: &str = "notifications/subscriptions/acknowledged";

/// The reserved `_meta` key every frame on a listen stream carries.
///
/// `SubscriptionsListenResultMeta["io.modelcontextprotocol/subscriptionId"]:
/// RequestId` is REQUIRED (not optional) on the result's `_meta`, and its value
/// "is the JSON-RPC ID of the `subscriptions/listen` request that opened the
/// stream (and equals this response's `id`)".
pub const SUBSCRIPTION_ID_META_KEY: &str = "io.modelcontextprotocol/subscriptionId";

/// The `_meta` key of a params/result object.
///
/// Spelled here rather than imported from [`crate::types::mrtr`] so this module
/// carries no dependency on the MRTR unit (which is `streamable-http`-gated),
/// and asserted equal to it by [`tests::meta_key_agrees_with_the_mrtr_spelling`].
const META_KEY: &str = "_meta";

/// The set of notification types a client asks a listen stream to deliver.
///
/// All four fields are OPTIONAL (`113-SPEC-RECHECK.md` § A.6):
///
/// ```typescript
/// // schema/draft/schema.ts:1270-1288
/// export interface SubscriptionFilter {
///   toolsListChanged?: boolean;
///   promptsListChanged?: boolean;
///   resourcesListChanged?: boolean;
///   resourceSubscriptions?: string[];   // "Replaces the former `resources/subscribe` RPC."
/// }
/// ```
///
/// # Examples
///
/// ```rust
/// use pmcp::types::subscriptions::SubscriptionFilter;
///
/// let mut filter = SubscriptionFilter::default();
/// filter.tools_list_changed = Some(true);
///
/// let wire = serde_json::to_value(&filter).unwrap();
/// assert_eq!(wire, serde_json::json!({ "toolsListChanged": true }));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionFilter {
    /// Deliver `notifications/tools/list_changed`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools_list_changed: Option<bool>,

    /// Deliver `notifications/prompts/list_changed`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts_list_changed: Option<bool>,

    /// Deliver `notifications/resources/list_changed`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources_list_changed: Option<bool>,

    /// The resource URIs to deliver `notifications/resources/updated` for.
    ///
    /// Replaces the former `resources/subscribe` RPC. The recorded type is
    /// `string[]` — an array of resource URIs — NOT a boolean and NOT a map.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_subscriptions: Option<Vec<String>>,
}

/// One notification type a listen stream can carry, as classified from a
/// [`ServerNotification`].
///
/// The absence of a variant for `notifications/progress` and
/// `notifications/message` is the WHOLE point: those are request-scoped and are
/// therefore excluded from every listen stream BY CONSTRUCTION rather than by a
/// runtime filter the caller supplies (spec D-12 RESOLUTION item 4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SubscriptionNotificationKind {
    /// `notifications/tools/list_changed`.
    ToolsListChanged,
    /// `notifications/prompts/list_changed`.
    PromptsListChanged,
    /// `notifications/resources/list_changed`.
    ResourcesListChanged,
    /// `notifications/resources/updated`, for the carried URI.
    ResourceUpdated(String),
}

/// Classify a [`ServerNotification`] as a subscription-deliverable kind, or
/// `None` when it can NEVER travel on a listen stream.
///
/// The match is EXHAUSTIVE over [`ServerNotification`], so adding a variant to
/// that enum is a compile-time tripwire here rather than a silent default into
/// either bucket.
pub(crate) fn subscription_kind_of(
    notification: &ServerNotification,
) -> Option<SubscriptionNotificationKind> {
    match notification {
        ServerNotification::ToolsChanged => Some(SubscriptionNotificationKind::ToolsListChanged),
        ServerNotification::PromptsChanged => {
            Some(SubscriptionNotificationKind::PromptsListChanged)
        },
        ServerNotification::ResourcesChanged => {
            Some(SubscriptionNotificationKind::ResourcesListChanged)
        },
        ServerNotification::ResourceUpdated(params) => Some(
            SubscriptionNotificationKind::ResourceUpdated(params.uri.clone()),
        ),
        // Request-scoped by construction: `progress` and `message` belong to the
        // request that asked for them and are never subscription-delivered.
        // `roots/list_changed` and `tasks/status` have no `SubscriptionFilter`
        // field at all, so no client can ever request them here.
        ServerNotification::Progress(_)
        | ServerNotification::LogMessage(_)
        | ServerNotification::RootsListChanged
        | ServerNotification::TaskStatus(_) => None,
    }
}

impl SubscriptionFilter {
    /// Does this filter request nothing at all?
    ///
    /// An empty AGREED filter means the server honored none of what was asked,
    /// which is still a conformant acknowledgement — the spec says an
    /// unsupported requested type "is omitted from this set".
    pub fn is_empty(&self) -> bool {
        !self.tools_list_changed.unwrap_or(false)
            && !self.prompts_list_changed.unwrap_or(false)
            && !self.resources_list_changed.unwrap_or(false)
            && self
                .resource_subscriptions
                .as_ref()
                .is_none_or(|uris| uris.is_empty())
    }

    /// The AGREED filter: the intersection of what the client REQUESTED and what
    /// this server's capabilities actually support.
    ///
    /// The result can never be a superset of `self` — every field is a
    /// conjunction with the requested value — which is the spec MUST that
    /// [`SubscriptionAcknowledgedParams::notifications`] reports.
    #[must_use]
    pub fn intersect_with_capabilities(&self, capabilities: &ServerCapabilities) -> Self {
        let tools = capabilities
            .tools
            .as_ref()
            .and_then(|c| c.list_changed)
            .unwrap_or(false);
        let prompts = capabilities
            .prompts
            .as_ref()
            .and_then(|c| c.list_changed)
            .unwrap_or(false);
        let resources_list = capabilities
            .resources
            .as_ref()
            .and_then(|c| c.list_changed)
            .unwrap_or(false);
        let resource_subscribe = capabilities
            .resources
            .as_ref()
            .and_then(|c| c.subscribe)
            .unwrap_or(false);

        Self {
            tools_list_changed: agreed_flag(self.tools_list_changed, tools),
            prompts_list_changed: agreed_flag(self.prompts_list_changed, prompts),
            resources_list_changed: agreed_flag(self.resources_list_changed, resources_list),
            resource_subscriptions: match (&self.resource_subscriptions, resource_subscribe) {
                (Some(uris), true) if !uris.is_empty() => Some(uris.clone()),
                _ => None,
            },
        }
    }

    /// Does this (already AGREED) filter cover `kind`?
    #[must_use]
    pub(crate) fn covers(&self, kind: &SubscriptionNotificationKind) -> bool {
        match kind {
            SubscriptionNotificationKind::ToolsListChanged => {
                self.tools_list_changed.unwrap_or(false)
            },
            SubscriptionNotificationKind::PromptsListChanged => {
                self.prompts_list_changed.unwrap_or(false)
            },
            SubscriptionNotificationKind::ResourcesListChanged => {
                self.resources_list_changed.unwrap_or(false)
            },
            SubscriptionNotificationKind::ResourceUpdated(uri) => self
                .resource_subscriptions
                .as_ref()
                .is_some_and(|uris| uris.iter().any(|candidate| candidate == uri)),
        }
    }
}

/// A requested flag survives into the agreed filter only when the server
/// supports it; an unsupported request is OMITTED (`None`), never `Some(false)`.
fn agreed_flag(requested: Option<bool>, supported: bool) -> Option<bool> {
    (requested.unwrap_or(false) && supported).then_some(true)
}

/// `subscriptions/listen` request parameters.
///
/// ```typescript
/// // schema/draft/schema.ts:1295-1302
/// export interface SubscriptionsListenRequestParams extends RequestParams {
///   notifications: SubscriptionFilter;   // REQUIRED
/// }
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SubscriptionsListenParams {
    /// The notification types the client asks to receive. REQUIRED.
    pub notifications: SubscriptionFilter,

    /// Per-request reserved metadata, spelled `_meta` on the wire.
    ///
    /// Carried the same way every other v2 request's params carry it (Phase-113
    /// D-113-A: the spec spelling on egress, the legacy `meta` accepted on
    /// ingress so a pre-113 pmcp peer still round-trips).
    #[serde(
        rename = "_meta",
        alias = "meta",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub meta: Option<Value>,
}

impl SubscriptionsListenParams {
    /// Build listen params requesting `notifications`.
    #[must_use]
    pub fn new(notifications: SubscriptionFilter) -> Self {
        Self {
            notifications,
            meta: None,
        }
    }
}

/// The params of the FIRST frame on a listen stream.
///
/// ```typescript
/// // schema/draft/schema.ts:1358-1366
/// export interface SubscriptionsAcknowledgedNotificationParams extends NotificationParams {
///   notifications: SubscriptionFilter;   // REQUIRED
/// }
/// ```
///
/// Ordering MUST, verbatim from the draft: "This notification MUST be the first
/// message the server sends carrying the subscription's ID in
/// `io.modelcontextprotocol/subscriptionId`. The server MUST NOT send any
/// notification on the subscription before acknowledging it."
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SubscriptionAcknowledgedParams {
    /// The subset of requested notification types the server AGREED to honor.
    ///
    /// Never a superset of what the client requested: an unsupported requested
    /// type is omitted from this set.
    pub notifications: SubscriptionFilter,

    /// Reserved metadata carrying [`SUBSCRIPTION_ID_META_KEY`].
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

impl SubscriptionAcknowledgedParams {
    /// Build an acknowledgement for `subscription_id` honoring `notifications`.
    #[must_use]
    pub fn new(notifications: SubscriptionFilter, subscription_id: &RequestId) -> Self {
        Self {
            notifications,
            meta: Some(subscription_id_meta(subscription_id)),
        }
    }
}

/// The graceful-teardown JSON-RPC result of a `subscriptions/listen` request.
///
/// `SubscriptionsListenResult extends Result { _meta: SubscriptionsListenResultMeta }`
/// where `_meta` is REQUIRED and carries a REQUIRED
/// `"io.modelcontextprotocol/subscriptionId": RequestId`.
///
/// `_meta` is modelled as an open map rather than a one-field struct so the
/// SERVER-OWNED reserved keys the shared v2 envelope adds
/// (`io.modelcontextprotocol/serverInfo`) survive a round-trip instead of being
/// dropped by a closed type.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SubscriptionsListenResult {
    /// Reserved metadata; REQUIRED, and always carries
    /// [`SUBSCRIPTION_ID_META_KEY`].
    #[serde(rename = "_meta")]
    pub meta: serde_json::Map<String, Value>,
}

impl SubscriptionsListenResult {
    /// The empty result closing the stream opened by `subscription_id`.
    #[must_use]
    pub fn new(subscription_id: &RequestId) -> Self {
        let mut meta = serde_json::Map::new();
        meta.insert(
            SUBSCRIPTION_ID_META_KEY.to_string(),
            request_id_value(subscription_id),
        );
        Self { meta }
    }

    /// The subscription id this result closes, if present.
    #[must_use]
    pub fn subscription_id(&self) -> Option<&Value> {
        self.meta.get(SUBSCRIPTION_ID_META_KEY)
    }
}

/// A `_meta` object carrying only [`SUBSCRIPTION_ID_META_KEY`].
#[must_use]
pub fn subscription_id_meta(subscription_id: &RequestId) -> Value {
    let mut meta = serde_json::Map::new();
    meta.insert(
        SUBSCRIPTION_ID_META_KEY.to_string(),
        request_id_value(subscription_id),
    );
    Value::Object(meta)
}

/// A [`RequestId`] as its wire JSON value (an untagged string or number).
#[must_use]
pub fn request_id_value(id: &RequestId) -> Value {
    match id {
        RequestId::String(s) => Value::String(s.clone()),
        RequestId::Number(n) => Value::Number((*n).into()),
    }
}

/// Tag an already-serialized notification's `params._meta` with
/// [`SUBSCRIPTION_ID_META_KEY`].
///
/// Every frame on a listen stream carries the subscription id, not just the
/// acknowledgement — spec D-12 RESOLUTION item 2. Creates `params` and
/// `params._meta` when absent; a non-object at either position is REPLACED, so a
/// frame can never reach the wire untagged.
pub(crate) fn tag_notification_with_subscription_id(
    frame: &mut Value,
    subscription_id: &RequestId,
) {
    let Some(object) = frame.as_object_mut() else {
        return;
    };
    if !matches!(object.get("params"), Some(Value::Object(_))) {
        object.insert("params".to_string(), Value::Object(serde_json::Map::new()));
    }
    let Some(params) = object.get_mut("params").and_then(Value::as_object_mut) else {
        return;
    };
    if !matches!(params.get(META_KEY), Some(Value::Object(_))) {
        params.insert(META_KEY.to_string(), Value::Object(serde_json::Map::new()));
    }
    if let Some(meta) = params.get_mut(META_KEY).and_then(Value::as_object_mut) {
        meta.insert(
            SUBSCRIPTION_ID_META_KEY.to_string(),
            request_id_value(subscription_id),
        );
    }
}

/// Does this server advertise ANY subscription-delivered capability?
///
/// # The conformance gating expression, verbatim
///
/// ```typescript
/// // github.com/modelcontextprotocol/conformance
/// // src/scenarios/server/stateless.ts:975-1015
/// //
/// // A server that advertises no subscription-delivered capability has
/// // nothing to serve on subscriptions/listen, so a -32601 there is a
/// // legitimate feature absence (SKIPPED). A server that DOES advertise
/// // listChanged/subscribe but rejects the method fails: it claims a
/// // feature it does not serve.
/// const advertisesSubscriptions = !!(
///   discoverCapabilities?.tools?.listChanged ||
///   discoverCapabilities?.prompts?.listChanged ||
///   discoverCapabilities?.resources?.listChanged ||
///   discoverCapabilities?.resources?.subscribe
/// );
/// ```
///
/// # Why ONE predicate
///
/// This is THE tripwire. The `server/discover` capability projection and the
/// `subscriptions/listen` route gate both read this single function, so the
/// advertisement and the implementation cannot drift: if a server advertises one
/// of the four, the gate serves the stream; if it advertises none, the gate
/// answers `-32601` and the conformance suite records SKIPPED. A second copy of
/// this expression anywhere is the defect this design exists to prevent.
///
/// # Examples
///
/// ```rust
/// use pmcp::types::subscriptions::advertises_subscriptions;
/// use pmcp::types::{ServerCapabilities, ToolCapabilities};
///
/// // pmcp's stateless enterprise DEFAULT advertises nothing subscription-
/// // delivered, for which answering `subscriptions/listen` with -32601 is
/// // conformant.
/// assert!(!advertises_subscriptions(&ServerCapabilities::default()));
///
/// let mut caps = ServerCapabilities::default();
/// caps.tools = Some(ToolCapabilities { list_changed: Some(true) });
/// assert!(advertises_subscriptions(&caps));
/// ```
#[must_use]
pub fn advertises_subscriptions(capabilities: &ServerCapabilities) -> bool {
    capabilities
        .tools
        .as_ref()
        .and_then(|c| c.list_changed)
        .unwrap_or(false)
        || capabilities
            .prompts
            .as_ref()
            .and_then(|c| c.list_changed)
            .unwrap_or(false)
        || capabilities
            .resources
            .as_ref()
            .and_then(|c| c.list_changed)
            .unwrap_or(false)
        || capabilities
            .resources
            .as_ref()
            .and_then(|c| c.subscribe)
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::capabilities::{PromptCapabilities, ResourceCapabilities, ToolCapabilities};
    use crate::types::notifications::{LogMessageParams, LoggingLevel, ResourceUpdatedParams};
    use serde_json::json;

    fn caps(
        tools: bool,
        prompts: bool,
        resources_list: bool,
        resources_sub: bool,
    ) -> ServerCapabilities {
        let mut capabilities = ServerCapabilities::default();
        if tools {
            capabilities.tools = Some(ToolCapabilities {
                list_changed: Some(true),
            });
        }
        if prompts {
            capabilities.prompts = Some(PromptCapabilities {
                list_changed: Some(true),
            });
        }
        if resources_list || resources_sub {
            capabilities.resources = Some(ResourceCapabilities {
                subscribe: resources_sub.then_some(true),
                list_changed: resources_list.then_some(true),
            });
        }
        capabilities
    }

    #[test]
    fn meta_key_agrees_with_the_mrtr_spelling() {
        // One `_meta` spelling across the crate: if the MRTR unit ever renames
        // it, this fails rather than the two silently diverging.
        assert_eq!(META_KEY, crate::types::mrtr::META_KEY);
    }

    #[test]
    fn filter_round_trips_the_literal_camel_case_spellings() {
        let filter = SubscriptionFilter {
            tools_list_changed: Some(true),
            prompts_list_changed: Some(true),
            resources_list_changed: Some(true),
            resource_subscriptions: Some(vec!["mem://a".to_string()]),
        };
        let wire = serde_json::to_value(&filter).expect("serializes");
        assert_eq!(
            wire,
            json!({
                "toolsListChanged": true,
                "promptsListChanged": true,
                "resourcesListChanged": true,
                "resourceSubscriptions": ["mem://a"],
            }),
            "the four wire keys are camelCase and spelled exactly as the schema declares"
        );
        let back: SubscriptionFilter = serde_json::from_value(wire).expect("round-trips");
        assert_eq!(back, filter);
    }

    #[test]
    fn an_absent_field_is_omitted_from_the_wire() {
        let wire = serde_json::to_value(SubscriptionFilter::default()).expect("serializes");
        assert_eq!(wire, json!({}), "no `null`s on the wire");
    }

    #[test]
    fn filter_matches_the_shape_recorded_in_the_spec_recheck() {
        // The example is built from `113-SPEC-RECHECK.md` § A.6, which
        // transcribed `schema/draft/schema.ts:1270-1288` verbatim:
        // `resourceSubscriptions?: string[]` — an ARRAY OF RESOURCE URIS.
        let recorded = json!({
            "toolsListChanged": true,
            "promptsListChanged": false,
            "resourcesListChanged": true,
            "resourceSubscriptions": ["file:///a.txt", "mem://b"],
        });
        let filter: SubscriptionFilter =
            serde_json::from_value(recorded).expect("the recorded example deserializes");
        assert_eq!(filter.tools_list_changed, Some(true));
        assert_eq!(filter.prompts_list_changed, Some(false));
        assert_eq!(filter.resources_list_changed, Some(true));
        assert_eq!(
            filter.resource_subscriptions.as_deref(),
            Some(&["file:///a.txt".to_string(), "mem://b".to_string()][..]),
            "resourceSubscriptions is `string[]`, not a bool and not a map"
        );
    }

    #[test]
    fn a_boolean_resource_subscriptions_is_rejected() {
        // The negative half of the checkpoint lock: if someone "fixes" the field
        // to a bool because prose suggested it, this test fails.
        let wrong = json!({ "resourceSubscriptions": true });
        assert!(
            serde_json::from_value::<SubscriptionFilter>(wrong).is_err(),
            "a boolean resourceSubscriptions must NOT deserialize"
        );
    }

    #[test]
    fn listen_params_require_the_notifications_field() {
        let params: SubscriptionsListenParams =
            serde_json::from_value(json!({ "notifications": { "toolsListChanged": true } }))
                .expect("deserializes");
        assert_eq!(params.notifications.tools_list_changed, Some(true));
        assert!(
            serde_json::from_value::<SubscriptionsListenParams>(json!({})).is_err(),
            "`notifications` is REQUIRED (no `?` in the schema declaration)"
        );
    }

    #[test]
    fn listen_params_read_both_meta_spellings() {
        // D-113-A: spec `_meta` on egress, legacy `meta` accepted on ingress.
        for key in ["_meta", "meta"] {
            let params: SubscriptionsListenParams = serde_json::from_value(json!({
                "notifications": {},
                key: { "io.modelcontextprotocol/protocolVersion": "2026-07-28" },
            }))
            .expect("deserializes");
            assert!(params.meta.is_some(), "`{key}` reaches the meta field");
        }
        let out = serde_json::to_value(SubscriptionsListenParams {
            notifications: SubscriptionFilter::default(),
            meta: Some(json!({ "k": 1 })),
        })
        .expect("serializes");
        assert!(
            out.get("_meta").is_some() && out.get("meta").is_none(),
            "egress uses the spec spelling only"
        );
    }

    #[test]
    fn acknowledged_params_carry_the_subscription_id() {
        let ack = SubscriptionAcknowledgedParams::new(
            SubscriptionFilter {
                tools_list_changed: Some(true),
                ..SubscriptionFilter::default()
            },
            &RequestId::Number(1),
        );
        let wire = serde_json::to_value(&ack).expect("serializes");
        assert_eq!(
            wire,
            json!({
                "notifications": { "toolsListChanged": true },
                "_meta": { SUBSCRIPTION_ID_META_KEY: 1 },
            })
        );
    }

    #[test]
    fn listen_result_is_empty_apart_from_the_required_meta() {
        let result = SubscriptionsListenResult::new(&RequestId::String("abc".to_string()));
        let wire = serde_json::to_value(&result).expect("serializes");
        assert_eq!(
            wire,
            json!({ "_meta": { SUBSCRIPTION_ID_META_KEY: "abc" } })
        );
        assert_eq!(result.subscription_id(), Some(&json!("abc")));
    }

    #[test]
    fn listen_result_meta_keeps_the_envelope_keys() {
        // The shared v2 envelope adds `io.modelcontextprotocol/serverInfo` into
        // `_meta`; an open map keeps it, a closed struct would drop it.
        let wire = json!({
            "_meta": {
                SUBSCRIPTION_ID_META_KEY: 7,
                "io.modelcontextprotocol/serverInfo": { "name": "s", "version": "1" },
            },
        });
        let result: SubscriptionsListenResult =
            serde_json::from_value(wire.clone()).expect("deserializes");
        assert_eq!(serde_json::to_value(&result).expect("re-serializes"), wire);
    }

    #[test]
    fn every_frame_is_tagged_with_the_subscription_id() {
        let mut frame = json!({
            "jsonrpc": "2.0",
            "method": "notifications/tools/list_changed",
        });
        tag_notification_with_subscription_id(&mut frame, &RequestId::Number(1));
        assert_eq!(frame["params"]["_meta"][SUBSCRIPTION_ID_META_KEY], json!(1));

        // A pre-existing `_meta` is MERGED into, not replaced.
        let mut frame = json!({
            "jsonrpc": "2.0",
            "method": "notifications/resources/updated",
            "params": { "uri": "mem://a", "_meta": { "keep": true } },
        });
        tag_notification_with_subscription_id(&mut frame, &RequestId::String("s".to_string()));
        assert_eq!(frame["params"]["uri"], json!("mem://a"));
        assert_eq!(frame["params"]["_meta"]["keep"], json!(true));
        assert_eq!(
            frame["params"]["_meta"][SUBSCRIPTION_ID_META_KEY],
            json!("s")
        );
    }

    #[test]
    fn advertises_subscriptions_over_all_sixteen_capability_combinations() {
        for bits in 0u8..16 {
            let tools = bits & 1 != 0;
            let prompts = bits & 2 != 0;
            let resources_list = bits & 4 != 0;
            let resources_sub = bits & 8 != 0;
            let expected = tools || prompts || resources_list || resources_sub;
            assert_eq!(
                advertises_subscriptions(&caps(tools, prompts, resources_list, resources_sub)),
                expected,
                "bits={bits:04b} (tools={tools}, prompts={prompts}, \
                 resourcesListChanged={resources_list}, resourcesSubscribe={resources_sub})"
            );
        }
    }

    #[test]
    fn a_false_capability_is_not_an_advertisement() {
        let mut capabilities = ServerCapabilities::default();
        capabilities.tools = Some(ToolCapabilities {
            list_changed: Some(false),
        });
        capabilities.resources = Some(ResourceCapabilities {
            subscribe: Some(false),
            list_changed: Some(false),
        });
        assert!(
            !advertises_subscriptions(&capabilities),
            "`listChanged: false` is falsy in the conformance expression too"
        );
    }

    #[test]
    fn the_agreed_filter_is_never_a_superset_of_the_request() {
        let requested = SubscriptionFilter {
            tools_list_changed: Some(true),
            ..SubscriptionFilter::default()
        };
        // Server supports EVERYTHING; the client asked for one thing.
        let agreed = requested.intersect_with_capabilities(&caps(true, true, true, true));
        assert_eq!(agreed.tools_list_changed, Some(true));
        assert_eq!(agreed.prompts_list_changed, None);
        assert_eq!(agreed.resources_list_changed, None);
        assert_eq!(agreed.resource_subscriptions, None);
    }

    #[test]
    fn an_unsupported_request_is_omitted_from_the_agreed_filter() {
        let requested = SubscriptionFilter {
            tools_list_changed: Some(true),
            prompts_list_changed: Some(true),
            resources_list_changed: Some(true),
            resource_subscriptions: Some(vec!["mem://a".to_string()]),
        };
        // Only tools.listChanged is supported.
        let agreed = requested.intersect_with_capabilities(&caps(true, false, false, false));
        assert_eq!(agreed.tools_list_changed, Some(true));
        assert_eq!(agreed.prompts_list_changed, None, "omitted, not `false`");
        assert_eq!(agreed.resources_list_changed, None);
        assert_eq!(agreed.resource_subscriptions, None);
        assert!(!agreed.is_empty());
    }

    #[test]
    fn an_entirely_unsupported_request_agrees_to_nothing() {
        let requested = SubscriptionFilter {
            prompts_list_changed: Some(true),
            ..SubscriptionFilter::default()
        };
        let agreed = requested.intersect_with_capabilities(&caps(true, false, false, false));
        assert!(agreed.is_empty());
        assert_eq!(
            serde_json::to_value(&agreed).expect("serializes"),
            json!({})
        );
    }

    #[test]
    fn resource_subscriptions_survive_only_with_the_subscribe_capability() {
        let requested = SubscriptionFilter {
            resource_subscriptions: Some(vec!["mem://a".to_string()]),
            ..SubscriptionFilter::default()
        };
        assert_eq!(
            requested
                .intersect_with_capabilities(&caps(false, false, false, true))
                .resource_subscriptions
                .as_deref(),
            Some(&["mem://a".to_string()][..])
        );
        assert_eq!(
            requested
                .intersect_with_capabilities(&caps(false, false, true, false))
                .resource_subscriptions,
            None,
            "resources.listChanged does NOT imply resources.subscribe"
        );
    }

    #[test]
    fn covers_matches_only_the_requested_kinds() {
        let agreed = SubscriptionFilter {
            tools_list_changed: Some(true),
            resource_subscriptions: Some(vec!["mem://a".to_string()]),
            ..SubscriptionFilter::default()
        };
        assert!(agreed.covers(&SubscriptionNotificationKind::ToolsListChanged));
        assert!(!agreed.covers(&SubscriptionNotificationKind::PromptsListChanged));
        assert!(!agreed.covers(&SubscriptionNotificationKind::ResourcesListChanged));
        assert!(
            agreed.covers(&SubscriptionNotificationKind::ResourceUpdated(
                "mem://a".to_string()
            ))
        );
        assert!(
            !agreed.covers(&SubscriptionNotificationKind::ResourceUpdated(
                "mem://other".to_string()
            )),
            "a URI the client did not name is not covered"
        );
    }

    #[test]
    fn request_scoped_notifications_have_no_subscription_kind() {
        use crate::types::ProgressNotification;
        use crate::types::ProgressToken;

        assert!(
            subscription_kind_of(&ServerNotification::Progress(ProgressNotification::new(
                ProgressToken::String("t".to_string()),
                1.0,
                None
            )))
            .is_none(),
            "`notifications/progress` is request-scoped and never subscription-delivered"
        );
        assert!(
            subscription_kind_of(&ServerNotification::LogMessage(LogMessageParams::new(
                LoggingLevel::Info,
                "hi"
            )))
            .is_none(),
            "`notifications/message` is request-scoped and never subscription-delivered"
        );
        assert!(subscription_kind_of(&ServerNotification::RootsListChanged).is_none());
    }

    #[test]
    fn subscription_deliverable_notifications_classify() {
        assert_eq!(
            subscription_kind_of(&ServerNotification::ToolsChanged),
            Some(SubscriptionNotificationKind::ToolsListChanged)
        );
        assert_eq!(
            subscription_kind_of(&ServerNotification::PromptsChanged),
            Some(SubscriptionNotificationKind::PromptsListChanged)
        );
        assert_eq!(
            subscription_kind_of(&ServerNotification::ResourcesChanged),
            Some(SubscriptionNotificationKind::ResourcesListChanged)
        );
        assert_eq!(
            subscription_kind_of(&ServerNotification::ResourceUpdated(
                ResourceUpdatedParams::new("mem://a")
            )),
            Some(SubscriptionNotificationKind::ResourceUpdated(
                "mem://a".to_string()
            ))
        );
    }
}
