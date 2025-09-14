/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use slog::Level;

use crate::types::{Facility, Priority, Severity};

pub(crate) fn level_to_severity(level: Level) -> Severity {
    match level {
        Level::Critical => Severity::Critical,
        Level::Error => Severity::Error,
        Level::Warning => Severity::Warning,
        Level::Info => Severity::Notice,
        Level::Debug => Severity::Info,
        Level::Trace => Severity::Debug,
    }
}

pub(crate) fn encode_priority(severity: Severity, facility: Facility) -> Priority {
    facility as u8 | severity as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_to_severity_all_levels() {
        assert!(matches!(
            level_to_severity(Level::Critical),
            Severity::Critical
        ));
        assert!(matches!(level_to_severity(Level::Error), Severity::Error));
        assert!(matches!(
            level_to_severity(Level::Warning),
            Severity::Warning
        ));
        assert!(matches!(level_to_severity(Level::Info), Severity::Notice));
        assert!(matches!(level_to_severity(Level::Debug), Severity::Info));
        assert!(matches!(level_to_severity(Level::Trace), Severity::Debug));
    }

    #[test]
    fn encode_priority_bit_operations() {
        let priority = encode_priority(Severity::Emergency, Facility::Kern);
        assert_eq!(priority, 0); // Kern (0<<3) | Emergency (0) = 0 | 0 = 0

        let priority = encode_priority(Severity::Alert, Facility::User);
        assert_eq!(priority, 1 << 3 | 1); // User (1<<3) | Alert (1) = 8 | 1 = 9

        let priority = encode_priority(Severity::Critical, Facility::Mail);
        assert_eq!(priority, 2 << 3 | 2); // Mail (2<<3) | Critical (2) = 16 | 2 = 18

        let priority = encode_priority(Severity::Error, Facility::Daemon);
        assert_eq!(priority, 3 << 3 | 3); // Daemon (3<<3) | Error (3) = 24 | 3 = 27

        let priority = encode_priority(Severity::Warning, Facility::AuthPrivate);
        assert_eq!(priority, 10 << 3 | 4); // AuthPrivate (10<<3) | Warning (4) = 80 | 4 = 84

        let priority = encode_priority(Severity::Notice, Facility::Local7);
        assert_eq!(priority, 23 << 3 | 5); // Local7 (23<<3) | Notice (5) = 184 | 5 = 189

        let priority = encode_priority(Severity::Info, Facility::Syslog);
        assert_eq!(priority, 5 << 3 | 6); // Syslog (5<<3) | Info (6) = 40 | 6 = 46

        let priority = encode_priority(Severity::Debug, Facility::Local0);
        assert_eq!(priority, 16 << 3 | 7); // Local0 (16<<3) | Debug (7) = 128 | 7 = 135
    }
}
