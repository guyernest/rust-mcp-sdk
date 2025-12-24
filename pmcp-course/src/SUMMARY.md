# Summary

[Introduction](./introduction.md)
[Prerequisites](./prerequisites.md)

---

# Part I: Foundations

- [The Enterprise Case for MCP](./part1-foundations/ch01-enterprise-case.md)
  - [The AI Integration Problem](./part1-foundations/ch01-01-problem.md)
  - [Why MCP Over Alternatives](./part1-foundations/ch01-02-why-mcp.md)
  - [Why Rust for Enterprise](./part1-foundations/ch01-03-why-rust.md)

- [Your First Production Server](./part1-foundations/ch02-first-server.md)
  - [Development Environment Setup](./part1-foundations/ch02-01-setup.md)
  - [Creating a Workspace with cargo pmcp](./part1-foundations/ch02-02-workspace.md)
  - [The Calculator Server](./part1-foundations/ch02-03-calculator.md)
  - [Understanding the Generated Code](./part1-foundations/ch02-04-code-walkthrough.md)
  - [Running with MCP Inspector](./part1-foundations/ch02-05-inspector.md)

- [Database MCP Servers](./part1-foundations/ch03-database-servers.md)
  - [The Enterprise Data Access Problem](./part1-foundations/ch03-01-data-access.md)
  - [Building db-explorer](./part1-foundations/ch03-02-db-explorer.md)
  - [SQL Safety and Injection Prevention](./part1-foundations/ch03-03-sql-safety.md)
  - [Resource-Based Data Patterns](./part1-foundations/ch03-04-resources.md)
  - [Handling Large Results](./part1-foundations/ch03-05-pagination.md)

---

# Part II: Thoughtful Design

- [Beyond Tool Sprawl](./part2-design/ch04-design-principles.md)
  - [The Anti-Pattern: 50 Confusing Tools](./part2-design/ch04-01-antipatterns.md)
  - [Cohesive API Design](./part2-design/ch04-02-cohesion.md)
  - [Single Responsibility for Tools](./part2-design/ch04-03-responsibility.md)

- [Input Validation and Output Schemas](./part2-design/ch05-validation.md)
  - [Schema-Driven Validation](./part2-design/ch05-01-input-validation.md)
  - [Output Schemas for Composition](./part2-design/ch05-02-output-schemas.md)
  - [Type-Safe Tool Annotations](./part2-design/ch05-03-annotations.md)

- [Resources, Prompts, and Workflows](./part2-design/ch06-beyond-tools.md)
  - [When to Use Resources vs Tools](./part2-design/ch06-01-resources-vs-tools.md)
  - [Prompts as Workflow Templates](./part2-design/ch06-02-prompts.md)
  - [Designing Multi-Step Workflows](./part2-design/ch06-03-workflows.md)

---

# Part III: Cloud Deployment

- [Deployment Overview](./part3-deployment/ch07-deployment-overview.md)
  - [Serverless vs Containers vs Edge](./part3-deployment/ch07-01-options.md)
  - [Cost Analysis Framework](./part3-deployment/ch07-02-costs.md)
  - [Security Boundaries](./part3-deployment/ch07-03-security.md)

- [AWS Lambda Deployment](./part3-deployment/ch08-aws-lambda.md)
  - [Lambda Architecture for MCP](./part3-deployment/ch08-01-architecture.md)
  - [Deploying with cargo pmcp](./part3-deployment/ch08-02-deploy.md)
  - [API Gateway Configuration](./part3-deployment/ch08-03-api-gateway.md)
  - [Cold Start Optimization](./part3-deployment/ch08-04-cold-starts.md)
  - [Connecting Clients](./part3-deployment/ch08-05-connecting.md)

- [Cloudflare Workers (WASM)](./part3-deployment/ch09-cloudflare.md)
  - [WASM Compilation](./part3-deployment/ch09-01-wasm.md)
  - [Edge Deployment Benefits](./part3-deployment/ch09-02-edge.md)
  - [Workers-Specific Considerations](./part3-deployment/ch09-03-workers.md)

- [Google Cloud Run](./part3-deployment/ch10-cloud-run.md)
  - [Container-Based Deployment](./part3-deployment/ch10-01-containers.md)
  - [Auto-Scaling Configuration](./part3-deployment/ch10-02-scaling.md)
  - [Comparison with Lambda](./part3-deployment/ch10-03-comparison.md)

---

# Part IV: Testing

- [Local Testing](./part4-testing/ch11-local-testing.md)
  - [MCP Inspector Deep Dive](./part4-testing/ch11-01-inspector.md)
  - [mcp-tester Introduction](./part4-testing/ch11-02-mcp-tester.md)
  - [Schema-Driven Test Generation](./part4-testing/ch11-03-schema-tests.md)

- [Remote Testing](./part4-testing/ch12-remote-testing.md)
  - [Testing Deployed Servers](./part4-testing/ch12-01-remote.md)
  - [CI/CD Integration](./part4-testing/ch12-02-cicd.md)
  - [Regression Testing](./part4-testing/ch12-03-regression.md)

---

# Part V: Enterprise Security

- [OAuth for MCP](./part5-security/ch13-oauth.md)
  - [Why OAuth, Not API Keys](./part5-security/ch13-01-why-oauth.md)
  - [OAuth 2.0 Fundamentals](./part5-security/ch13-02-oauth-basics.md)
  - [Token Validation](./part5-security/ch13-03-validation.md)

- [Identity Provider Integration](./part5-security/ch14-providers.md)
  - [AWS Cognito](./part5-security/ch14-01-cognito.md)
  - [Auth0](./part5-security/ch14-02-auth0.md)
  - [Microsoft Entra ID](./part5-security/ch14-03-entra.md)
  - [Multi-Tenant Considerations](./part5-security/ch14-04-multitenant.md)

---

# Part VI: AI-Assisted Development

- [Using AI to Build MCP Servers](./part6-ai-dev/ch15-ai-assisted.md)
  - [Claude Code CLI](./part6-ai-dev/ch15-01-claude-cli.md)
  - [IDE Integration](./part6-ai-dev/ch15-02-ide.md)
  - [AI Agent Instructions](./part6-ai-dev/ch15-03-agents.md)
  - [Kiro as Alternative](./part6-ai-dev/ch15-04-kiro.md)

- [Effective AI Collaboration](./part6-ai-dev/ch16-collaboration.md)
  - [Prompting for Business Logic](./part6-ai-dev/ch16-01-prompting.md)
  - [Review and Refinement](./part6-ai-dev/ch16-02-review.md)
  - [Quality Assurance](./part6-ai-dev/ch16-03-qa.md)

---

# Part VII: Observability

- [Middleware and Instrumentation](./part7-observability/ch17-middleware.md)
  - [Middleware Architecture](./part7-observability/ch17-01-architecture.md)
  - [Logging Best Practices](./part7-observability/ch17-02-logging.md)
  - [Metrics Collection](./part7-observability/ch17-03-metrics.md)

- [Operations and Monitoring](./part7-observability/ch18-operations.md)
  - [pmcp.run Dashboard](./part7-observability/ch18-01-pmcp-run.md)
  - [Alerting and Incidents](./part7-observability/ch18-02-alerting.md)
  - [Performance Optimization](./part7-observability/ch18-03-performance.md)

---

# Part VIII: Advanced Patterns

- [Server Composition](./part8-advanced/ch19-composition.md)
  - [Foundation Servers](./part8-advanced/ch19-01-foundations.md)
  - [Domain Servers](./part8-advanced/ch19-02-domains.md)
  - [Orchestration Patterns](./part8-advanced/ch19-03-orchestration.md)

- [MCP Applications](./part8-advanced/ch20-applications.md)
  - [Building UIs for MCP](./part8-advanced/ch20-01-ui.md)
  - [High Availability](./part8-advanced/ch20-02-ha.md)
  - [Migration Strategies](./part8-advanced/ch20-03-migration.md)

---

# Appendices

- [Appendix A: cargo pmcp Reference](./appendix/cargo-pmcp-reference.md)
- [Appendix B: Template Gallery](./appendix/template-gallery.md)
- [Appendix C: Troubleshooting](./appendix/troubleshooting.md)
- [Appendix D: Security Checklist](./appendix/security-checklist.md)
