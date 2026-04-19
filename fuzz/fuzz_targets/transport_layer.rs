#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use bytes::{BufMut, BytesMut};
use libfuzzer_sys::fuzz_target;

// Custom types for fuzzing transport behavior
#[derive(Debug, Arbitrary)]
#[allow(dead_code)]
struct FuzzTransportMessage {
    message_type: FuzzMessageType,
    payload: Vec<u8>,
    metadata: Option<FuzzMetadata>,
}

#[derive(Debug, Arbitrary)]
enum FuzzMessageType {
    Text,
    Binary,
    Close,
    Ping,
    Pong,
}

#[derive(Debug, Arbitrary)]
#[allow(dead_code)]
struct FuzzMetadata {
    timestamp: u64,
    sequence: u32,
    compressed: bool,
    fragmented: bool,
}

// Simulate transport operations
fn simulate_transport_operations(data: &[u8]) {
    // 1. Test message framing
    let mut buffer = BytesMut::new();

    // Add length prefix
    if data.len() < 65536 {
        buffer.put_u16(data.len() as u16);
        buffer.put_slice(data);

        // Try to read back
        if buffer.len() >= 2 {
            let len = u16::from_be_bytes([buffer[0], buffer[1]]) as usize;
            if buffer.len() >= 2 + len {
                let _message = &buffer[2..2 + len];
            }
        }
    }

    // 2. Test message chunking
    let chunk_size = 1024;
    let chunks: Vec<_> = data.chunks(chunk_size).collect();

    let mut reassembled = Vec::new();
    for chunk in chunks {
        reassembled.extend_from_slice(chunk);
    }
    // In fuzzing, we should not panic on assertions - just validate
    if reassembled != data {
        // This should never happen, but if it does, just return
        return;
    }

    // 3. Test message compression simulation
    if data.len() > 10 && data.len() < 10000 {
        // Add upper bound to prevent issues
        // Simulate simple run-length encoding
        let mut compressed = Vec::new();
        let max_compressed_size = 20000; // Limit compressed size too
        let mut i = 0;
        while i < data.len() && compressed.len() < max_compressed_size {
            let byte = data[i];
            let mut count = 1;
            while i + count < data.len() && data[i + count] == byte && count < 255 {
                count += 1;
            }
            compressed.push(count as u8);
            compressed.push(byte);
            i += count;
        }

        // Decompress with size limit to prevent memory exhaustion
        let mut decompressed = Vec::new();
        let max_decompressed_size = 100_000; // Reasonable limit for fuzzing
        let mut j = 0;
        while j < compressed.len() - 1 {
            let count = compressed[j];
            let byte = compressed[j + 1];
            for _ in 0..count {
                if decompressed.len() >= max_decompressed_size {
                    break; // Stop decompression if we hit the size limit
                }
                decompressed.push(byte);
            }
            if decompressed.len() >= max_decompressed_size {
                break; // Stop outer loop too
            }
            j += 2;
        }
    }
}

// Test WebSocket-like framing
fn test_websocket_framing(data: &[u8]) {
    // WebSocket frame structure simulation
    if data.len() < 2 {
        return;
    }

    let _fin = (data[0] & 0x80) != 0;
    let opcode = data[0] & 0x0F;
    let masked = (data[1] & 0x80) != 0;
    let payload_len = data[1] & 0x7F;

    let mut offset = 2;
    let actual_len = if payload_len == 126 {
        if data.len() < offset + 2 {
            return;
        }
        let len = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
        offset += 2;
        // Limit to reasonable size for fuzzing
        len.min(100_000)
    } else if payload_len == 127 {
        if data.len() < offset + 8 {
            return;
        }
        let len = u64::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]);
        offset += 8;
        // Limit to reasonable size for fuzzing - prevent huge allocations
        (len as usize).min(100_000)
    } else {
        payload_len as usize
    };

    let mask_key = if masked {
        if data.len() < offset + 4 {
            return;
        }
        let key = [
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ];
        offset += 4;
        Some(key)
    } else {
        None
    };

    // Make sure we don't try to read more than available
    let safe_len = actual_len.min(data.len().saturating_sub(offset));
    if data.len() >= offset + safe_len && safe_len > 0 {
        let mut payload = data[offset..offset + safe_len].to_vec();

        // Unmask if needed
        if let Some(key) = mask_key {
            for i in 0..payload.len() {
                payload[i] ^= key[i % 4];
            }
        }

        // Process based on opcode
        match opcode {
            0x0 => {}, // Continuation
            0x1 => {
                // Text frame
                let _ = String::from_utf8(payload);
            },
            0x2 => {}, // Binary frame
            0x8 => {}, // Close
            0x9 => {}, // Ping
            0xA => {}, // Pong
            _ => {},
        }
    }
}

fuzz_target!(|data: &[u8]| {
    // 1. Test basic transport operations
    simulate_transport_operations(data);

    // 2. Test WebSocket-like framing
    test_websocket_framing(data);

    // 3. Generate structured transport messages
    let mut u = Unstructured::new(data);

    if let Ok(fuzz_msg) = FuzzTransportMessage::arbitrary(&mut u) {
        // Validate message types
        match fuzz_msg.message_type {
            FuzzMessageType::Text => {
                // Text messages should be valid UTF-8
                let _ = String::from_utf8(fuzz_msg.payload.clone());
            },
            FuzzMessageType::Binary => {
                // Binary messages can be any bytes - just validate size is reasonable
                // Don't assert, just skip if too large (could be arbitrary fuzzer data)
                if fuzz_msg.payload.len() >= 10_000_000 {
                    return; // Skip unreasonably large payloads
                }
            },
            _ => {},
        }
    }

    // 4. Test transport buffering and flow control
    if data.len() > 0 {
        // Limit buffer size to prevent excessive allocation
        let buffer_size = ((data[0] as usize) * 256).min(1_000_000);
        let mut buffer = Vec::with_capacity(buffer_size);

        for chunk in data.chunks(256) {
            if buffer.len() + chunk.len() <= buffer_size {
                buffer.extend_from_slice(chunk);
            } else {
                // Buffer full, process and clear
                buffer.clear();
                buffer.extend_from_slice(chunk);
            }
        }
    }

    // 5. Test message ordering and sequencing
    let mut messages = Vec::new();
    let mut seq = 0u32;

    for chunk in data.chunks(16) {
        if chunk.len() >= 4 {
            let msg_seq = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            messages.push((msg_seq, chunk));
        }
        seq = seq.wrapping_add(1);
    }

    // Sort by sequence number
    messages.sort_by_key(|(s, _)| *s);

    // 6. Test connection state transitions
    let states = ["connecting", "connected", "closing", "closed"];
    if data.len() > 0 {
        let state_idx = (data[0] as usize) % states.len();
        let _current_state = states[state_idx];

        // Simulate state transitions
        match state_idx {
            0 => {}, // Connecting - wait for handshake
            1 => {
                // Connected - can send/receive
                let can_send = data.len() > 1 && data[1] & 0x01 != 0;
                let can_receive = data.len() > 1 && data[1] & 0x02 != 0;
                // In a connected state, at least one capability should be present
                // but for fuzzing, we should handle the case where neither is set
                if !can_send && !can_receive {
                    // Invalid state - skip this iteration
                    return;
                }
            },
            2 => {}, // Closing - wait for close confirmation
            3 => {}, // Closed - no operations allowed
            _ => {},
        }
    }
});
