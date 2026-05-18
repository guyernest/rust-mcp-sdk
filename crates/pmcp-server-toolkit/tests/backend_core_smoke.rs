//! D-03 backend-core smoke test (TKIT-08 anchor).
//!
//! Replays the construction surface each pmcp-run backend core uses today by
//! building a `pmcp::Server` from the open-images fixture through the
//! toolkit's public API alone. This stands in for cross-repo verification
//! per Phase 83 CONTEXT.md D-03.
//!
//! Per Phase 83 review R3, the second test
//! ([`backend_core_minimum_imports_compile`]) re-asserts the D-15 headline
//! DX promise: every symbol the smoke test constructs must be importable
//! from the crate root. If a symbol stops resolving at
//! `pmcp_server_toolkit::*`, this file fails to compile.
//!
//! Per Phase 83 review R7, the construction sequence uses the `try_*`
//! variants ([`ServerBuilderExt::try_tools_from_config`] /
//! [`ServerBuilderExt::try_code_mode_from_config`]) so misconfiguration
//! surfaces as a `Result`, not a panic.

#![cfg(all(not(target_arch = "wasm32"), feature = "code-mode"))]

use std::sync::Arc;

// PER REVIEW R3 — SINGLE crate-root import line. If this list grows to need
// module-path imports (`pmcp_server_toolkit::auth::*`), the D-15 headline DX
// promise is broken.
use pmcp_server_toolkit::{
    prompt_handlers_from_config, ServerBuilderExt, ServerConfig, StaticAuthProvider,
    StaticPromptHandler, StaticResourceHandler,
};

use pmcp::Server;

#[tokio::test]
async fn backend_core_construction_surface_smoke() {
    // The open-images fixture declares `token_secret = "${CODE_MODE_SECRET}"`
    // (operator-side shell interpolation, NOT the toolkit's `env:VAR_NAME`
    // form — see Plan 06 `resolve_token_secret`). Rather than modify the
    // fixture (forbidden by Plan 08 instructions), we re-point the parsed
    // [code_mode] `token_secret` to a toolkit-supported `env:` reference and
    // populate that env var. This exercises the exact same R9 enforcement
    // path the production builder takes and keeps the on-disk fixture
    // verbatim.
    std::env::set_var("PMCP_TOOLKIT_TOKEN_SECRET", "smoke-test-secret-do-not-use-in-prod");

    let toml = include_str!("fixtures/open-images-config.toml");
    let mut cfg = ServerConfig::from_toml_strict_validated(toml)
        .expect("open-images config must parse + validate");

    if let Some(cm) = cfg.code_mode.as_mut() {
        cm.token_secret = Some("env:PMCP_TOOLKIT_TOKEN_SECRET".to_string());
    }

    // Build using EVERY toolkit public surface (try_* variants per review R7).
    let mut builder = Server::builder()
        .name(&cfg.server.name)
        .version(&cfg.server.version)
        .try_tools_from_config(&cfg)
        .expect("synthesize tools from open-images")
        .try_code_mode_from_config(&cfg)
        .expect("wire code-mode from open-images")
        .resources_arc(Arc::new(StaticResourceHandler::from(&cfg)))
        .auth_provider_arc(Arc::new(StaticAuthProvider::new(
            "smoke-test-bearer-token-do-not-use-in-prod",
        )));

    // Wire every configured [[prompts]] entry via prompt_arc — exercises
    // the TKIT-05 construction surface (StaticPromptHandler) end-to-end.
    for (name, handler) in prompt_handlers_from_config(&cfg) {
        builder = builder.prompt_arc(name, Arc::new(handler));
    }
    // Reference the crate-root re-export so the import is exercised even
    // when the fixture has zero [[prompts]] entries.
    let _ = StaticPromptHandler::from(&cfg);

    let server = builder
        .build()
        .expect("Server::builder().build() must succeed against the open-images fixture");

    assert!(
        !cfg.tools.is_empty(),
        "open-images fixture must declare at least one [[tools]] entry"
    );
    let first_tool_name = &cfg.tools[0].name;
    assert!(
        server.get_tool(first_tool_name).is_some(),
        "After try_tools_from_config(), Server::get_tool('{}') must return Some \
         — Phase 82 BLDR-04 surface verification",
        first_tool_name
    );
}

#[tokio::test]
async fn backend_core_minimum_imports_compile() {
    // Per review R3: compile-only assertion proving every public symbol the
    // backend-core re-export shim (Plan 09 shim-pmcp-run-shared) references
    // is importable from the crate root WITHOUT module-path qualification.
    //
    // If a re-export is missing OR a consumer would need
    // `pmcp_server_toolkit::auth::X` instead of `pmcp_server_toolkit::X`,
    // this test fails to compile and the D-15 headline DX promise is broken.

    use pmcp_server_toolkit::{
        AuthProvider, ConfigValidationError, ConnectorError, Dialect, EnvSecrets, SecretValue,
        SecretsProvider, ServerBuilderExt, ServerConfig, SqlConnector, StaticAuthProvider,
        StaticPromptHandler, StaticResourceHandler,
    };

    #[cfg(feature = "code-mode")]
    use pmcp_server_toolkit::code_mode::{
        ApprovalToken, CodeExecutor, HmacTokenGenerator, NoopPolicyEvaluator, TokenSecret,
        ValidationPipeline,
    };

    // Reference each path via PhantomData / sized types via Option to assert
    // the symbol resolves at the crate root — no runtime allocations needed.
    // Per review R3, the import block above does NOT use `as _` (that would
    // hide a missing re-export). Concrete-typed references below force the
    // compiler to resolve every path.
    let _ = (
        std::marker::PhantomData::<dyn AuthProvider>,
        std::marker::PhantomData::<ConfigValidationError>,
        std::marker::PhantomData::<ConnectorError>,
        std::marker::PhantomData::<Dialect>,
        std::marker::PhantomData::<EnvSecrets>,
        std::marker::PhantomData::<SecretValue>,
        std::marker::PhantomData::<dyn SecretsProvider>,
        std::marker::PhantomData::<ServerConfig>,
        std::marker::PhantomData::<dyn SqlConnector>,
        std::marker::PhantomData::<StaticAuthProvider>,
        std::marker::PhantomData::<StaticPromptHandler>,
        std::marker::PhantomData::<StaticResourceHandler>,
    );
    // `ServerBuilderExt: Sized` so it can't be `dyn`; reference a method
    // pointer instead — equivalent crate-root path assertion.
    let _: fn(
        pmcp::ServerBuilder,
        &ServerConfig,
    ) -> pmcp_server_toolkit::Result<pmcp::ServerBuilder> =
        <pmcp::ServerBuilder as ServerBuilderExt>::try_tools_from_config;

    #[cfg(feature = "code-mode")]
    let _ = (
        std::marker::PhantomData::<ApprovalToken>,
        std::marker::PhantomData::<Box<dyn CodeExecutor>>,
        std::marker::PhantomData::<HmacTokenGenerator>,
        std::marker::PhantomData::<NoopPolicyEvaluator>,
        std::marker::PhantomData::<TokenSecret>,
        std::marker::PhantomData::<ValidationPipeline>,
    );
}
