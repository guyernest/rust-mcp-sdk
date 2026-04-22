//! Example: Dynamic Client Registration (RFC 7591) with `OAuthHelper`.
//!
//! Demonstrates how a library user can build an OAuth client that
//! auto-registers itself with any server advertising a `registration_endpoint`
//! via OIDC discovery, without hardcoding a `client_id`. This is the
//! SDK-side companion to `cargo pmcp auth login --client <name>`.
//!
//! Run with:
//!   cargo run --example c08_oauth_dcr --features oauth
//!
//! This example does NOT require network access: it constructs the
//! `OAuthConfig`, prints what DCR would do, and exits. A live end-to-end
//! invocation requires a real MCP server with a `registration_endpoint`.

#![cfg(feature = "oauth")]

use pmcp::client::oauth::{OAuthConfig, OAuthHelper};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== PMCP SDK — Dynamic Client Registration example ===\n");

    let config = OAuthConfig {
        issuer: None,
        mcp_server_url: Some("https://mcp.example.com".to_string()),
        // client_id = None + dcr_enabled = true => SDK performs RFC 7591 DCR
        client_id: None,
        client_name: Some("my-cool-app".to_string()),
        dcr_enabled: true,
        scopes: vec!["openid".to_string()],
        cache_file: None,
        redirect_port: 8080,
    };

    println!("Constructed OAuthConfig:");
    println!("  mcp_server_url  = {:?}", config.mcp_server_url);
    println!(
        "  client_id       = {:?}  (None => DCR will fire)",
        config.client_id
    );
    println!("  client_name     = {:?}", config.client_name);
    println!("  dcr_enabled     = {}", config.dcr_enabled);
    println!("  redirect_port   = {}", config.redirect_port);
    println!();
    println!("At `get_access_token().await`, OAuthHelper will:");
    println!("  1. GET /.well-known/openid-configuration on mcp_server_url");
    println!("  2. If registration_endpoint is advertised AND client_id.is_none():");
    println!("     POST to registration_endpoint with RFC 7591 public-PKCE body");
    println!("     (including response_types: [\"code\"] per RFC 7591 §3.1)");
    println!("     Capture the returned client_id for the PKCE flow");
    println!("  3. Run the PKCE authorization code flow with the resolved client_id");
    println!();

    // Sanity-check that OAuthHelper accepts the config shape.
    let _helper = OAuthHelper::new(config)?;
    println!(
        "OAuthHelper::new(..) succeeded. (Drive with .get_access_token().await \
         to invoke DCR + PKCE.)"
    );
    println!();
    println!(
        "For cache-persisting callers (refresh_token + expires_at across runs), \
         use OAuthHelper::authorize_with_details() instead of get_access_token()."
    );
    Ok(())
}
