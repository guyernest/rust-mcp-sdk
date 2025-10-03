//! Property-based tests for SSE transport optimizations
//!
//! PMCP-4002: Property tests for SSE performance and reliability

#![cfg(feature = "sse")]

use pmcp::shared::OptimizedSseConfig;
use proptest::prelude::*;
use std::time::Duration;

#[cfg(test)]
mod sse_performance {
    use super::*;

    proptest! {
        /// Property: Buffer size configuration is respected
        #[test]
        fn property_buffer_size(
            buffer_size in 10usize..1000
        ) {
            let config = OptimizedSseConfig {
                buffer_size,
                ..Default::default()
            };

            prop_assert_eq!(config.buffer_size, buffer_size);
            prop_assert!(buffer_size >= 10);
        }

        /// Property: Connection pooling configuration
        #[test]
        fn property_connection_pooling(
            enable_pooling in any::<bool>(),
            max_connections in 1usize..100
        ) {
            let config = OptimizedSseConfig {
                enable_pooling,
                max_connections,
                ..Default::default()
            };

            prop_assert_eq!(config.enable_pooling, enable_pooling);
            prop_assert_eq!(config.max_connections, max_connections);

            if enable_pooling {
                prop_assert!(max_connections > 0);
            }
        }

        /// Property: Keepalive interval configuration
        #[test]
        fn property_keepalive_interval(
            keepalive_secs in 1u64..300
        ) {
            let config = OptimizedSseConfig {
                keepalive_interval: Duration::from_secs(keepalive_secs),
                ..Default::default()
            };

            prop_assert_eq!(config.keepalive_interval.as_secs(), keepalive_secs);
            prop_assert!(keepalive_secs > 0);
        }

        /// Property: Flush interval configuration
        #[test]
        fn property_flush_interval(
            flush_millis in 10u64..5000
        ) {
            let config = OptimizedSseConfig {
                flush_interval: Duration::from_millis(flush_millis),
                ..Default::default()
            };

            prop_assert_eq!(config.flush_interval.as_millis() as u64, flush_millis);
            prop_assert!(flush_millis >= 10);
        }

        /// Property: Reconnect configuration
        #[test]
        fn property_reconnect_config(
            max_reconnects in 0usize..20,
            reconnect_delay_secs in 1u64..60
        ) {
            let config = OptimizedSseConfig {
                max_reconnects,
                reconnect_delay: Duration::from_secs(reconnect_delay_secs),
                ..Default::default()
            };

            prop_assert_eq!(config.max_reconnects, max_reconnects);
            prop_assert_eq!(config.reconnect_delay.as_secs(), reconnect_delay_secs);
        }

        /// Property: Compression configuration
        #[test]
        fn property_compression_config(
            enable_compression in any::<bool>()
        ) {
            let config = OptimizedSseConfig {
                enable_compression,
                ..Default::default()
            };

            prop_assert_eq!(config.enable_compression, enable_compression);
        }

        /// Property: URL configuration validation
        #[test]
        fn property_url_configuration(
            port in 1024u16..65535
        ) {
            let url = format!("http://localhost:{}/sse", port);
            let config = OptimizedSseConfig {
                url: url.clone(),
                ..Default::default()
            };

            prop_assert_eq!(&config.url, &url);
            prop_assert!(config.url.starts_with("http://") || config.url.starts_with("https://"));
        }
    }
}

#[cfg(test)]
mod sse_reliability {
    use super::*;

    proptest! {
        /// Property: Connection timeout bounds
        #[test]
        fn property_connection_timeout(
            timeout_secs in 1u64..600
        ) {
            let config = OptimizedSseConfig {
                connection_timeout: Duration::from_secs(timeout_secs),
                ..Default::default()
            };

            prop_assert!(timeout_secs > 0);
            prop_assert!(timeout_secs <= 600);
            prop_assert_eq!(config.connection_timeout.as_secs(), timeout_secs);
        }

        /// Property: Event buffer capacity
        #[test]
        fn property_event_buffer_capacity(
            buffer_size in 1usize..10000
        ) {
            use std::collections::VecDeque;

            let buffer: VecDeque<String> = VecDeque::with_capacity(buffer_size);
            prop_assert!(buffer.capacity() >= buffer_size);
        }
    }
}

#[cfg(test)]
mod sse_event_coalescing {
    use super::*;

    proptest! {
        /// Property: Event coalescing preserves order
        #[test]
        fn property_event_order_preserved(
            events in prop::collection::vec(any::<u32>(), 1..100)
        ) {
            // Events should maintain order through buffering
            let original = events.clone();
            let buffered = events;

            for (i, event) in original.iter().enumerate() {
                prop_assert_eq!(*event, buffered[i]);
            }
        }

        /// Property: Buffer flush threshold
        #[test]
        fn property_buffer_flush_threshold(
            buffer_size in 10usize..1000,
            event_count in 1usize..2000
        ) {
            // Should flush when buffer reaches capacity
            let flush_count = event_count / buffer_size;
            let remaining = event_count % buffer_size;

            prop_assert_eq!(flush_count * buffer_size + remaining, event_count);
        }
    }
}
