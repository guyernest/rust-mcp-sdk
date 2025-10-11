//! Simple resource implementations with builder pattern support.

use crate::types::{Content, ListResourcesResult, ReadResourceResult, ResourceInfo};
use crate::Result;
use async_trait::async_trait;
use base64::Engine;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use super::cancellation::RequestHandlerExtra;
use super::dynamic_resources::{DynamicResourceProvider, RequestContext, ResourceRouter};
use super::ResourceHandler;

/// A static resource that returns fixed content.
#[derive(Debug, Clone)]
pub struct StaticResource {
    uri: String,
    name: String,
    description: Option<String>,
    mime_type: Option<String>,
    content: Content,
}

impl StaticResource {
    /// Create a new static resource with URI and text content.
    pub fn new_text(uri: impl Into<String>, content: impl Into<String>) -> Self {
        let uri = uri.into();
        let name = uri.rsplit('/').next().unwrap_or(&uri).to_string();

        Self {
            uri,
            name,
            description: None,
            mime_type: Some("text/plain".to_string()),
            content: Content::Text {
                text: content.into(),
            },
        }
    }

    /// Create a new static resource with URI and image content.
    pub fn new_image(uri: impl Into<String>, data: &[u8], mime_type: impl Into<String>) -> Self {
        let uri = uri.into();
        let name = uri.rsplit('/').next().unwrap_or(&uri).to_string();
        let mime_type = mime_type.into();

        Self {
            uri,
            name,
            description: None,
            mime_type: Some(mime_type.clone()),
            content: Content::Image {
                data: base64::prelude::BASE64_STANDARD.encode(data),
                mime_type,
            },
        }
    }

    /// Set the resource name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set the resource description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the MIME type.
    pub fn with_mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = Some(mime_type.into());
        self
    }

    /// Get the resource info.
    pub fn info(&self) -> ResourceInfo {
        ResourceInfo {
            uri: self.uri.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            mime_type: self.mime_type.clone(),
        }
    }

    /// Get the resource URI.
    pub fn uri(&self) -> &str {
        &self.uri
    }
}

/// A collection of resources that can be managed together.
///
/// Supports both static resources (fixed URIs) and dynamic providers (URI templates).
/// Static resources are checked first for O(1) lookup, then dynamic providers are
/// tried in priority order.
pub struct ResourceCollection {
    resources: HashMap<String, Arc<StaticResource>>,
    router: ResourceRouter,
}

impl fmt::Debug for ResourceCollection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResourceCollection")
            .field(
                "static_resources",
                &self.resources.keys().collect::<Vec<_>>(),
            )
            .field("dynamic_providers", &self.router.all_templates().len())
            .finish()
    }
}

impl Default for ResourceCollection {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceCollection {
    /// Create a new empty resource collection.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::server::simple_resources::ResourceCollection;
    ///
    /// let collection = ResourceCollection::new();
    /// ```
    pub fn new() -> Self {
        Self {
            resources: HashMap::new(),
            router: ResourceRouter::new(),
        }
    }

    /// Add a static resource to the collection.
    ///
    /// Static resources are checked first for O(1) lookup performance.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::server::simple_resources::{ResourceCollection, StaticResource};
    ///
    /// let collection = ResourceCollection::new()
    ///     .add_resource(StaticResource::new_text("file://readme.txt", "Hello world"));
    /// ```
    pub fn add_resource(mut self, resource: StaticResource) -> Self {
        self.resources
            .insert(resource.uri.clone(), Arc::new(resource));
        self
    }

    /// Add a static resource by reference (avoids cloning).
    pub fn add_static(mut self, resource: StaticResource) -> Self {
        self.resources
            .insert(resource.uri.clone(), Arc::new(resource));
        self
    }

    /// Add multiple static resources to the collection.
    pub fn add_resources(mut self, resources: Vec<StaticResource>) -> Self {
        for resource in resources {
            self.resources
                .insert(resource.uri.clone(), Arc::new(resource));
        }
        self
    }

    /// Add a dynamic resource provider to the collection.
    ///
    /// Dynamic providers handle URI patterns using RFC 6570 templates.
    /// They are checked after static resources, in priority order.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::server::simple_resources::ResourceCollection;
    /// use pmcp::server::dynamic_resources::{DynamicResourceProvider, UriParams, RequestContext};
    /// use pmcp::types::{Content, ReadResourceResult, ResourceTemplate};
    /// use async_trait::async_trait;
    /// use std::sync::Arc;
    ///
    /// struct DatasetProvider;
    ///
    /// #[async_trait]
    /// impl DynamicResourceProvider for DatasetProvider {
    ///     fn templates(&self) -> Vec<ResourceTemplate> {
    ///         vec![ResourceTemplate {
    ///             uri_template: "datasets://{id}/schema".parse().unwrap(),
    ///             name: "Dataset Schema".to_string(),
    ///             description: Some("Schema for a dataset".to_string()),
    ///             mime_type: Some("application/json".to_string()),
    ///         }]
    ///     }
    ///
    ///     async fn fetch(
    ///         &self,
    ///         _uri: &str,
    ///         params: UriParams,
    ///         _context: RequestContext,
    ///     ) -> pmcp::Result<ReadResourceResult> {
    ///         let id = params.get("id").unwrap();
    ///         Ok(ReadResourceResult {
    ///             contents: vec![Content::Text {
    ///                 text: format!("Schema for dataset {}", id),
    ///             }],
    ///         })
    ///     }
    /// }
    ///
    /// let collection = ResourceCollection::new()
    ///     .add_dynamic_provider(Arc::new(DatasetProvider));
    /// ```
    pub fn add_dynamic_provider(mut self, provider: Arc<dyn DynamicResourceProvider>) -> Self {
        self.router.add_provider(provider);
        self
    }

    /// Get a static resource by URI.
    pub fn get(&self, uri: &str) -> Option<&Arc<StaticResource>> {
        self.resources.get(uri)
    }

    /// List all resources (static and dynamic templates).
    pub fn list(&self) -> Vec<ResourceInfo> {
        let mut infos: Vec<ResourceInfo> = self
            .resources
            .values()
            .map(|resource| resource.info())
            .collect();

        // Add dynamic templates as resources
        for template in self.router.all_templates() {
            infos.push(ResourceInfo {
                uri: template.uri_template.to_string(),
                name: template.name,
                description: template.description,
                mime_type: template.mime_type,
            });
        }

        infos
    }
}

#[async_trait]
impl ResourceHandler for ResourceCollection {
    async fn read(&self, uri: &str, extra: RequestHandlerExtra) -> Result<ReadResourceResult> {
        // Try static resources first (fast O(1) lookup)
        if let Some(resource) = self.resources.get(uri) {
            return Ok(ReadResourceResult {
                contents: vec![resource.content.clone()],
            });
        }

        // Try dynamic providers (pattern matching)
        if let Some(matched) = self.router.match_uri(uri) {
            let context = RequestContext::new(extra);
            return matched.provider.fetch(uri, matched.params, context).await;
        }

        // Not found
        Err(crate::Error::protocol(
            crate::ErrorCode::INVALID_PARAMS,
            format!("Resource not found: {}", uri),
        ))
    }

    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> Result<ListResourcesResult> {
        Ok(ListResourcesResult {
            resources: self.list(),
            next_cursor: None,
        })
    }
}

/// A dynamic resource handler that uses callbacks.
pub struct DynamicResourceHandler<R, L>
where
    R: Fn(
            &str,
            RequestHandlerExtra,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<ReadResourceResult>> + Send>,
        > + Send
        + Sync,
    L: Fn(
            Option<String>,
            RequestHandlerExtra,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<ListResourcesResult>> + Send>,
        > + Send
        + Sync,
{
    read_handler: R,
    list_handler: L,
}

impl<R, L> fmt::Debug for DynamicResourceHandler<R, L>
where
    R: Fn(
            &str,
            RequestHandlerExtra,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<ReadResourceResult>> + Send>,
        > + Send
        + Sync,
    L: Fn(
            Option<String>,
            RequestHandlerExtra,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<ListResourcesResult>> + Send>,
        > + Send
        + Sync,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DynamicResourceHandler")
            .field("read_handler", &"<function>")
            .field("list_handler", &"<function>")
            .finish()
    }
}

impl<R, L> DynamicResourceHandler<R, L>
where
    R: Fn(
            &str,
            RequestHandlerExtra,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<ReadResourceResult>> + Send>,
        > + Send
        + Sync,
    L: Fn(
            Option<String>,
            RequestHandlerExtra,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<ListResourcesResult>> + Send>,
        > + Send
        + Sync,
{
    /// Create a new dynamic resource handler.
    pub fn new(read_handler: R, list_handler: L) -> Self {
        Self {
            read_handler,
            list_handler,
        }
    }
}

#[async_trait]
impl<R, L> ResourceHandler for DynamicResourceHandler<R, L>
where
    R: Fn(
            &str,
            RequestHandlerExtra,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<ReadResourceResult>> + Send>,
        > + Send
        + Sync,
    L: Fn(
            Option<String>,
            RequestHandlerExtra,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<ListResourcesResult>> + Send>,
        > + Send
        + Sync,
{
    async fn read(&self, uri: &str, extra: RequestHandlerExtra) -> Result<ReadResourceResult> {
        (self.read_handler)(uri, extra).await
    }

    async fn list(
        &self,
        cursor: Option<String>,
        extra: RequestHandlerExtra,
    ) -> Result<ListResourcesResult> {
        (self.list_handler)(cursor, extra).await
    }
}
