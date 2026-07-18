//! Connector-ref → endpoint map (D-16).
//!
//! An `AgentPackage`'s [`ComponentRef`](pmcp_package::ComponentRef) connectors
//! carry only a name/version/type — NOT an endpoint. The host supplies the
//! endpoint at resolve time via [`SlotResolver::resolve_endpoint`], and
//! [`build_endpoint_map`] assembles the full name → endpoint map the
//! [`ResolvedAgentConfig`](super::ResolvedAgentConfig) carries.

use std::collections::HashMap;

use pmcp_package::ComponentRef;

use super::resolver::{ResolveError, SlotResolver};

/// Build the connector-name → endpoint map for a package's connectors.
///
/// Each connector's endpoint is resolved through `resolver`; a connector with no
/// configured endpoint yields [`ResolveError::MissingEndpoint`].
///
/// # Errors
///
/// Propagates the first [`ResolveError`] from `resolve_endpoint`.
pub async fn build_endpoint_map(
    connectors: &[ComponentRef],
    resolver: &dyn SlotResolver,
) -> Result<HashMap<String, String>, ResolveError> {
    let mut map = HashMap::with_capacity(connectors.len());
    for connector in connectors {
        let name = connector.name();
        let endpoint = resolver.resolve_endpoint(name).await?;
        map.insert(name.to_string(), endpoint);
    }
    Ok(map)
}
