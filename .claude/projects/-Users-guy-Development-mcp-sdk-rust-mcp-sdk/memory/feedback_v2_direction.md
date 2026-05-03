---
name: v2.0 SDK Direction
description: User's strategic direction for the v2.0 release — focus on streamable HTTP, stateless, tasks with polling. De-prioritize SSE, elicitations, notifications.
type: feedback
---

Focus SDK on streamable HTTP and stateless calls. Tasks with polling for status is the way forward.

**Why:** SSE, elicitations, and notifications add complexity with little benefit. Streamable HTTP + stateless + task polling is the cleaner, more scalable architecture.

**How to apply:** When planning v2.0 phases, prioritize protocol version upgrade (2025-11-25) for Tasks support, conformance testing, and framework adapters. De-prioritize or skip SSE enhancements, elicitation features, and notification subscriptions. The v2.0 bump enables breaking changes — use this opportunity for massive protocol cleanup.
