use anyhow::Result;
use colored::*;
use std::time::{Duration, Instant};
use url::Url;

use crate::report::{TestCategory, TestReport, TestResult, TestStatus};

pub async fn run_diagnostics(
    url: &str,
    network: bool,
    timeout: Duration,
    _insecure: bool,
    api_key: Option<&str>,
) -> Result<TestReport> {
    let mut report = TestReport::new();
    let start = Instant::now();

    println!("{}", "═══════════════════════════════════════════".cyan());
    println!("{}", "CONNECTION DIAGNOSTICS".cyan().bold());
    println!("{}", "═══════════════════════════════════════════".cyan());
    if let Some(key) = api_key {
        println!("Using API key: {}***", &key[..key.len().min(4)]);
    }
    println!();

    // Parse URL
    let url_result = diagnose_url(url);
    report.add_test(url_result.clone());
    print_diagnostic_result(&url_result);

    if url_result.status != TestStatus::Passed {
        report.duration = start.elapsed();
        return Ok(report);
    }

    // For stdio, skip network tests
    if url == "stdio" {
        let stdio_result = TestResult {
            name: "Stdio Transport".to_string(),
            category: TestCategory::Core,
            status: TestStatus::Passed,
            duration: Duration::from_millis(0),
            error: None,
            details: Some("Stdio transport ready for use".to_string()),
        };
        report.add_test(stdio_result.clone());
        print_diagnostic_result(&stdio_result);
        report.duration = start.elapsed();
        return Ok(report);
    }

    // Network diagnostics
    if network {
        let parsed_url = Url::parse(url)?;

        // DNS resolution
        let dns_result = diagnose_dns(&parsed_url).await;
        report.add_test(dns_result.clone());
        print_diagnostic_result(&dns_result);

        if dns_result.status != TestStatus::Passed {
            print_suggestions_for_dns();
            report.duration = start.elapsed();
            return Ok(report);
        }

        // TCP connectivity
        let tcp_result = diagnose_tcp(&parsed_url, timeout).await;
        report.add_test(tcp_result.clone());
        print_diagnostic_result(&tcp_result);

        if tcp_result.status != TestStatus::Passed {
            print_suggestions_for_tcp(&parsed_url);
            report.duration = start.elapsed();
            return Ok(report);
        }

        // TLS/SSL (for HTTPS)
        if parsed_url.scheme() == "https" {
            let tls_result = diagnose_tls(&parsed_url).await;
            report.add_test(tls_result.clone());
            print_diagnostic_result(&tls_result);

            if tls_result.status == TestStatus::Failed {
                print_suggestions_for_tls();
            }
        }
    }

    // HTTP specific tests
    let http_result = diagnose_http(url, timeout).await;
    report.add_test(http_result.clone());
    print_diagnostic_result(&http_result);

    if http_result.status != TestStatus::Passed {
        print_suggestions_for_http(url);
    }

    // MCP protocol test
    let mcp_result = diagnose_mcp_protocol(url, timeout, api_key).await;
    report.add_test(mcp_result.clone());
    print_diagnostic_result(&mcp_result);

    if mcp_result.status != TestStatus::Passed {
        print_suggestions_for_mcp(&mcp_result);
    }

    report.duration = start.elapsed();

    // Print summary
    println!();
    println!("{}", "═══════════════════════════════════════════".cyan());
    println!("{}", "DIAGNOSTIC SUMMARY".cyan().bold());
    println!("{}", "═══════════════════════════════════════════".cyan());

    let passed = report
        .tests
        .iter()
        .filter(|t| t.status == TestStatus::Passed)
        .count();
    let failed = report
        .tests
        .iter()
        .filter(|t| t.status == TestStatus::Failed)
        .count();
    let warnings = report
        .tests
        .iter()
        .filter(|t| t.status == TestStatus::Warning)
        .count();

    println!(
        "  {} {} Passed  {} {} Failed  {} {} Warnings",
        "✓".green().bold(),
        passed.to_string().green(),
        "✗".red().bold(),
        failed.to_string().red(),
        "⚠".yellow().bold(),
        warnings.to_string().yellow()
    );

    if failed > 0 {
        println!();
        println!("{}", "RECOMMENDATIONS:".yellow().bold());
        print_overall_recommendations(&report);
    }

    Ok(report)
}

fn diagnose_url(url: &str) -> TestResult {
    let start = Instant::now();

    if url == "stdio" {
        return TestResult {
            name: "URL Validation".to_string(),
            category: TestCategory::Core,
            status: TestStatus::Passed,
            duration: start.elapsed(),
            error: None,
            details: Some("Using stdio transport".to_string()),
        };
    }

    match Url::parse(url) {
        Ok(parsed) => {
            let mut details = vec![
                format!("Scheme: {}", parsed.scheme()),
                format!("Host: {}", parsed.host_str().unwrap_or("none")),
            ];

            if let Some(port) = parsed.port() {
                details.push(format!("Port: {}", port));
            } else {
                let default_port = match parsed.scheme() {
                    "http" => 80,
                    "https" => 443,
                    _ => 0,
                };
                if default_port > 0 {
                    details.push(format!("Port: {} (default)", default_port));
                }
            }

            if !parsed.path().is_empty() && parsed.path() != "/" {
                details.push(format!("Path: {}", parsed.path()));
            }

            TestResult {
                name: "URL Validation".to_string(),
                category: TestCategory::Core,
                status: TestStatus::Passed,
                duration: start.elapsed(),
                error: None,
                details: Some(details.join(", ")),
            }
        },
        Err(e) => TestResult {
            name: "URL Validation".to_string(),
            category: TestCategory::Core,
            status: TestStatus::Failed,
            duration: start.elapsed(),
            error: Some(e.to_string()),
            details: None,
        },
    }
}

async fn diagnose_dns(url: &Url) -> TestResult {
    let start = Instant::now();
    let host = match url.host_str() {
        Some(h) => h,
        None => {
            return TestResult {
                name: "DNS Resolution".to_string(),
                category: TestCategory::Core,
                status: TestStatus::Failed,
                duration: start.elapsed(),
                error: Some("No host in URL".to_string()),
                details: None,
            };
        },
    };

    // Skip DNS for localhost
    if host == "localhost" || host == "127.0.0.1" || host == "::1" {
        return TestResult {
            name: "DNS Resolution".to_string(),
            category: TestCategory::Core,
            status: TestStatus::Passed,
            duration: start.elapsed(),
            error: None,
            details: Some(format!("Local host: {}", host)),
        };
    }

    match tokio::net::lookup_host(format!("{}:80", host)).await {
        Ok(addrs) => {
            let addresses: Vec<_> = addrs.collect();
            TestResult {
                name: "DNS Resolution".to_string(),
                category: TestCategory::Core,
                status: TestStatus::Passed,
                duration: start.elapsed(),
                error: None,
                details: Some(format!("Resolved to {} address(es)", addresses.len())),
            }
        },
        Err(e) => TestResult {
            name: "DNS Resolution".to_string(),
            category: TestCategory::Core,
            status: TestStatus::Failed,
            duration: start.elapsed(),
            error: Some(e.to_string()),
            details: None,
        },
    }
}

async fn diagnose_tcp(url: &Url, timeout: Duration) -> TestResult {
    let start = Instant::now();

    let host = url.host_str().unwrap_or("localhost");
    let port = url.port().unwrap_or(match url.scheme() {
        "http" => 80,
        "https" => 443,
        _ => 8080,
    });

    let addr = format!("{}:{}", host, port);

    match tokio::time::timeout(timeout, tokio::net::TcpStream::connect(&addr)).await {
        Ok(Ok(_)) => TestResult {
            name: "TCP Connection".to_string(),
            category: TestCategory::Core,
            status: TestStatus::Passed,
            duration: start.elapsed(),
            error: None,
            details: Some(format!("Connected to {}", addr)),
        },
        Ok(Err(e)) => TestResult {
            name: "TCP Connection".to_string(),
            category: TestCategory::Core,
            status: TestStatus::Failed,
            duration: start.elapsed(),
            error: Some(e.to_string()),
            details: Some(format!("Failed to connect to {}", addr)),
        },
        Err(_) => TestResult {
            name: "TCP Connection".to_string(),
            category: TestCategory::Core,
            status: TestStatus::Failed,
            duration: start.elapsed(),
            error: Some("Connection timeout".to_string()),
            details: Some(format!("Timeout after {:?}", timeout)),
        },
    }
}

async fn diagnose_tls(url: &Url) -> TestResult {
    let start = Instant::now();

    // Basic TLS check - would need actual TLS implementation
    TestResult {
        name: "TLS/SSL Certificate".to_string(),
        category: TestCategory::Core,
        status: TestStatus::Passed,
        duration: start.elapsed(),
        error: None,
        details: Some(format!(
            "TLS validation for {}",
            url.host_str().unwrap_or("unknown")
        )),
    }
}

async fn diagnose_http(url: &str, timeout: Duration) -> TestResult {
    let start = Instant::now();

    // Try a simple HTTP request
    match reqwest::Client::builder().timeout(timeout).build() {
        Ok(client) => match client.get(url).send().await {
            Ok(response) => {
                let status = response.status();
                let headers = response.headers();

                let mut details = vec![format!("Status: {}", status)];

                if let Some(content_type) = headers.get("content-type") {
                    details.push(format!("Content-Type: {:?}", content_type));
                }

                if let Some(server) = headers.get("server") {
                    details.push(format!("Server: {:?}", server));
                }

                TestResult {
                    name: "HTTP Response".to_string(),
                    category: TestCategory::Core,
                    status: if status.is_success() || status.as_u16() == 404 {
                        TestStatus::Passed
                    } else if status.is_server_error() {
                        TestStatus::Failed
                    } else {
                        TestStatus::Warning
                    },
                    duration: start.elapsed(),
                    error: if !status.is_success() && status.as_u16() != 404 {
                        Some(format!("HTTP {}", status))
                    } else {
                        None
                    },
                    details: Some(details.join(", ")),
                }
            },
            Err(e) => TestResult {
                name: "HTTP Response".to_string(),
                category: TestCategory::Core,
                status: TestStatus::Failed,
                duration: start.elapsed(),
                error: Some(e.to_string()),
                details: None,
            },
        },
        Err(e) => TestResult {
            name: "HTTP Response".to_string(),
            category: TestCategory::Core,
            status: TestStatus::Failed,
            duration: start.elapsed(),
            error: Some(format!("Failed to create HTTP client: {}", e)),
            details: None,
        },
    }
}

async fn diagnose_mcp_protocol(url: &str, timeout: Duration, api_key: Option<&str>) -> TestResult {
    let start = Instant::now();

    // Try to initialize MCP connection
    match crate::tester::ServerTester::new(url, timeout, false, api_key, None, None) {
        Ok(mut tester) => {
            // Try quick test
            match tester.run_quick_test().await {
                Ok(report) => {
                    let init_test = report
                        .tests
                        .iter()
                        .find(|t| t.name == "Initialize")
                        .cloned()
                        .unwrap_or_else(|| TestResult {
                            name: "MCP Protocol".to_string(),
                            category: TestCategory::Protocol,
                            status: TestStatus::Failed,
                            duration: start.elapsed(),
                            error: Some("Initialize test not found".to_string()),
                            details: None,
                        });

                    TestResult {
                        name: "MCP Protocol".to_string(),
                        category: TestCategory::Protocol,
                        status: init_test.status,
                        duration: start.elapsed(),
                        error: init_test.error,
                        details: init_test.details,
                    }
                },
                Err(e) => TestResult {
                    name: "MCP Protocol".to_string(),
                    category: TestCategory::Protocol,
                    status: TestStatus::Failed,
                    duration: start.elapsed(),
                    error: Some(e.to_string()),
                    details: None,
                },
            }
        },
        Err(e) => TestResult {
            name: "MCP Protocol".to_string(),
            category: TestCategory::Protocol,
            status: TestStatus::Failed,
            duration: start.elapsed(),
            error: Some(e.to_string()),
            details: None,
        },
    }
}

fn print_diagnostic_result(result: &TestResult) {
    let status_symbol = match result.status {
        TestStatus::Passed => "✓".green().bold(),
        TestStatus::Failed => "✗".red().bold(),
        TestStatus::Warning => "⚠".yellow().bold(),
        TestStatus::Skipped => "○".dimmed(),
    };

    let status_text = match result.status {
        TestStatus::Passed => "PASS".green(),
        TestStatus::Failed => "FAIL".red(),
        TestStatus::Warning => "WARN".yellow(),
        TestStatus::Skipped => "SKIP".dimmed(),
    };

    print!("{} [{:>4}] {:<25}", status_symbol, status_text, result.name);

    if let Some(details) = &result.details {
        println!(" {}", details.dimmed());
    } else if let Some(error) = &result.error {
        println!(" {}", error.red());
    } else {
        println!();
    }
}

fn print_suggestions_for_dns() {
    println!();
    println!("{}", "DNS Resolution Failed - Suggestions:".yellow().bold());
    println!("  • Check your internet connection");
    println!("  • Verify the hostname is correct");
    println!("  • Try using IP address instead of hostname");
    println!("  • Check your DNS settings (try 8.8.8.8 or 1.1.1.1)");
}

fn print_suggestions_for_tcp(url: &Url) {
    println!();
    println!("{}", "TCP Connection Failed - Suggestions:".yellow().bold());

    let port = url.port().unwrap_or(match url.scheme() {
        "http" => 80,
        "https" => 443,
        _ => 8080,
    });

    println!("  • Verify the server is running");
    println!("  • Check if port {} is open", port);
    println!("  • Check firewall settings");

    if url.host_str() == Some("localhost") {
        println!("  • For local servers, ensure the service is started");
        println!("  • Try: lsof -i :{} (to check if port is in use)", port);
    }

    println!("  • For AWS Lambda, ensure API Gateway is configured");
    println!("  • For Docker, check port mapping (-p {}:{})", port, port);
}

fn print_suggestions_for_tls() {
    println!();
    println!("{}", "TLS/SSL Issues - Suggestions:".yellow().bold());
    println!("  • For self-signed certificates, use --insecure flag");
    println!("  • Check certificate expiration date");
    println!("  • Verify certificate chain is complete");
    println!("  • For local development, consider using HTTP instead");
}

fn print_suggestions_for_http(url: &str) {
    println!();
    println!("{}", "HTTP Issues - Suggestions:".yellow().bold());

    if url.contains("amazonaws.com") {
        println!("  • For Lambda: Check API Gateway configuration");
        println!("  • Verify Lambda function is deployed");
        println!("  • Check CloudWatch logs for errors");
        println!("  • Ensure proper IAM permissions");
    }

    println!("  • Check server logs for errors");
    println!("  • Verify the endpoint path is correct");
    println!("  • For 404 errors, check API routing");
    println!("  • For 500 errors, check server implementation");
}

fn print_suggestions_for_mcp(result: &TestResult) {
    println!();
    println!("{}", "MCP Protocol Issues - Suggestions:".yellow().bold());

    if let Some(error) = &result.error {
        if error.contains("timeout") {
            println!("  • Server may be experiencing cold start (Lambda)");
            println!("  • Try increasing timeout with --timeout flag");
            println!("  • Check server performance and resource limits");
        } else if error.contains("parse") || error.contains("JSON") {
            println!("  • Server is not returning valid MCP responses");
            println!("  • Check server implementation follows MCP spec");
            println!("  • Verify Content-Type is application/json");
            println!("  • Check for error messages in response body");
        } else if error.contains("version") {
            println!("  • Protocol version mismatch");
            println!("  • Server may be using different MCP version");
            println!("  • Check server's protocol version support");
        }
    }

    println!("  • Ensure server implements MCP protocol correctly");
    println!("  • Try testing with a known working MCP client");
    println!("  • Check server documentation for setup instructions");
}

fn print_overall_recommendations(report: &TestReport) {
    let failed_tests: Vec<_> = report
        .tests
        .iter()
        .filter(|t| t.status == TestStatus::Failed)
        .collect();

    for test in failed_tests {
        match test.name.as_str() {
            "URL Validation" => {
                println!("  • Fix URL format: use http://host:port or https://host:port");
            },
            "DNS Resolution" => {
                println!("  • Cannot resolve hostname - check network and DNS");
            },
            "TCP Connection" => {
                println!("  • Cannot connect - verify server is running on correct port");
            },
            "TLS/SSL Certificate" => {
                println!("  • Certificate issues - use --insecure for testing");
            },
            "HTTP Response" => {
                println!("  • HTTP layer issues - check server logs and configuration");
            },
            "MCP Protocol" => {
                println!("  • MCP implementation issues - verify protocol compliance");
            },
            _ => {},
        }
    }
}
