//! Attachment wiring: turn an [`crate::compose::derive::AttachmentSet`] into an
//! attached, running reference-server stack.
//!
//! This layer cfg-gates each attachment branch behind the aggregate `runtime`
//! feature and fail-closes on unknown/uncompiled opt-in server names. Empty
//! documented seam — implemented in 109-06.
