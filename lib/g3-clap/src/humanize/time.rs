/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;
use std::time::Duration;

use anyhow::anyhow;
use clap::ArgMatches;

pub fn get_duration(args: &ArgMatches, id: &str) -> anyhow::Result<Option<Duration>> {
    if let Some(v) = args.get_one::<String>(id) {
        if let Ok(timeout) = humanize_rs::duration::parse(v) {
            Ok(Some(timeout))
        } else if let Ok(timeout) = u64::from_str(v) {
            Ok(Some(Duration::from_secs(timeout)))
        } else if let Ok(timeout) = f64::from_str(v) {
            let timeout = Duration::try_from_secs_f64(timeout)
                .map_err(|e| anyhow!("out of range timeout value: {e}"))?;
            Ok(Some(timeout))
        } else {
            Err(anyhow!("invalid {id} value {v}"))
        }
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Arg, ArgAction, Command};

    // Helper function to create ArgMatches for testing
    fn create_args(value: Option<&str>) -> ArgMatches {
        let arg = Arg::new("time").long("time").action(ArgAction::Set);
        let command = Command::new("test").arg(arg);

        if let Some(v) = value {
            command.get_matches_from(vec!["test", &format!("--time={v}")])
        } else {
            command.get_matches_from(vec!["test"])
        }
    }

    #[test]
    fn get_duration_none() {
        // No time argument provided
        let args = create_args(None);
        assert!(get_duration(&args, "time").unwrap().is_none());
    }

    #[test]
    fn get_duration_humanize_valid() {
        // Valid human-readable duration strings
        let cases = vec![
            ("1ns", Duration::new(0, 1)),
            ("1ms", Duration::new(0, 1_000_000)),
            ("1s", Duration::new(1, 0)),
            ("2m", Duration::new(120, 0)),
            ("3h", Duration::new(10800, 0)),
            ("1d", Duration::new(86400, 0)),
        ];

        for (input, expected) in cases {
            let args = create_args(Some(input));
            assert_eq!(get_duration(&args, "time").unwrap(), Some(expected));
        }
    }

    #[test]
    fn get_duration_integer_valid() {
        // Valid integer seconds
        let cases = vec![
            ("0", Duration::new(0, 0)),
            ("1", Duration::new(1, 0)),
            ("18446744073709551615", Duration::new(u64::MAX, 0)),
        ];

        for (input, expected) in cases {
            let args = create_args(Some(input));
            assert_eq!(get_duration(&args, "time").unwrap(), Some(expected));
        }
    }

    #[test]
    fn get_duration_float_valid() {
        // Valid float seconds
        let cases = vec![
            ("0.5", Duration::new(0, 500_000_000)),
            ("1.5", Duration::new(1, 500_000_000)),
            ("0.000001", Duration::new(0, 1000)), // 1μs
        ];

        for (input, expected) in cases {
            let args = create_args(Some(input));
            assert_eq!(get_duration(&args, "time").unwrap(), Some(expected));
        }
    }

    #[test]
    fn get_duration_float_invalid() {
        // Float seconds out of range
        let cases = vec![
            "1e309", // exceeds f64 max
            "-1.0",  // negative value
            "inf",   // infinity
            "NaN",   // not a number
        ];

        for input in cases {
            let args = create_args(Some(input));
            assert!(get_duration(&args, "time").is_err());
        }
    }

    #[test]
    fn get_duration_invalid() {
        // Invalid inputs
        let cases = vec![
            "abc",   // non-numeric
            "1.2.3", // invalid float
            "123a",  // trailing chars
        ];

        for input in cases {
            let args = create_args(Some(input));
            assert!(get_duration(&args, "time").is_err());
        }
    }
}
