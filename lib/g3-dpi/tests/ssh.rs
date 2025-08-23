/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_dpi::{
    MaybeProtocol, Protocol, ProtocolInspectError, ProtocolInspectionConfig, ProtocolInspector,
};

mod client {
    use super::*;

    fn check_client(data: &[u8]) -> Result<Protocol, ProtocolInspectError> {
        let mut inspector = ProtocolInspector::default();
        inspector.push_protocol(MaybeProtocol::Ssh);
        let config = ProtocolInspectionConfig::default();
        inspector.check_client_initial_data(&config, 22, data)
    }

    #[test]
    fn valid_ssh2_client() {
        const DATA: &[u8] = b"SSH-2.0-OpenSSH_8.9p1 Debian-10+deb10u2\r\n";
        let protocol = check_client(DATA).unwrap();
        assert_eq!(protocol, Protocol::Ssh);
    }

    #[test]
    fn valid_ssh199_client() {
        const DATA: &[u8] = b"SSH-1.99-OpenSSH_8.9p1 Debian-10+deb10u2\n";
        let protocol = check_client(DATA).unwrap();
        assert_eq!(protocol, Protocol::Ssh);
    }

    #[test]
    fn valid_ssh19_legacy_client() {
        const DATA: &[u8] = b"SSH-1.9-OpenSSH_8.9p1 Debian-10+deb10u2\r\n";
        let protocol = check_client(DATA).unwrap();
        assert_eq!(protocol, Protocol::SshLegacy);
    }

    #[test]
    fn valid_ssh1_legacy_client() {
        const DATA: &[u8] = b"SSH-1.5-OpenSSH_7.9p1 Debian-10+deb10u2\r\n";
        let protocol = check_client(DATA).unwrap();
        assert_eq!(protocol, Protocol::SshLegacy);
    }

    #[test]
    fn insufficient_data() {
        const DATA: &[u8] = b"SSH-2.";
        let result = check_client(DATA);
        assert!(matches!(result, Err(ProtocolInspectError::NeedMoreData(5))));
    }

    #[test]
    fn invalid_first_byte() {
        let mut data = vec![0; 200];
        data[0..8].copy_from_slice(b"XSH-2.0-");
        data[198..200].copy_from_slice(b"\r\n");
        let protocol = check_client(&data).unwrap();
        assert_eq!(protocol, Protocol::Unknown);
    }

    #[test]
    fn invalid_prefix() {
        const DATA: &[u8] = b"SXH-2.0-OpenSSH_8.9p1\r\n";
        let protocol = check_client(DATA).unwrap();
        assert_eq!(protocol, Protocol::Unknown);
    }

    #[test]
    fn invalid_version() {
        let data = b"SSH-1.X-OpenSSH_8.9p1\r\n";
        let protocol = check_client(data).unwrap();
        assert_eq!(protocol, Protocol::Unknown);

        let data = b"SSH-2.1-OpenSSH_8.9p1\r\n";
        let protocol = check_client(data).unwrap();
        assert_eq!(protocol, Protocol::Unknown);

        let data = b"SSH-3.0-OpenSSH_8.9p1\r\n";
        let protocol = check_client(data).unwrap();
        assert_eq!(protocol, Protocol::Unknown);
    }

    #[test]
    fn missing_hyphen_after_version() {
        const DATA: &[u8] = b"SSH-2.0XOpenSSH_8.9p1\r\n";
        let protocol = check_client(DATA).unwrap();
        assert_eq!(protocol, Protocol::Unknown);
    }

    #[test]
    fn missing_crlf() {
        let data = b"SSH-2.0-OpenSSH_8.9p1";
        let result = check_client(data);
        assert!(matches!(result, Err(ProtocolInspectError::NeedMoreData(1))));

        let data = b"SSH-2.0-OpenSSH_8.9p1\n";
        let protocol = check_client(data).unwrap();
        assert_eq!(protocol, Protocol::Unknown);
    }

    #[test]
    fn max_length_no_newline() {
        let mut data = vec![0; 255];
        data[0..4].copy_from_slice(b"SSH-");
        data[4..8].copy_from_slice(b"2.0-");
        let protocol = check_client(&data).unwrap();
        assert_eq!(protocol, Protocol::Unknown);
    }
}

mod server {
    use super::*;

    fn check_server(data: &[u8]) -> Result<Protocol, ProtocolInspectError> {
        let mut inspector = ProtocolInspector::default();
        inspector.push_protocol(MaybeProtocol::Ssh);
        let config = ProtocolInspectionConfig::default();
        inspector.check_server_initial_data(&config, 22, data)
    }

    #[test]
    fn valid_ssh2_server() {
        const DATA: &[u8] = b"SSH-2.0-OpenSSH_7.9p1 Debian-10+deb10u2\r\n";
        let protocol = check_server(DATA).unwrap();
        assert_eq!(protocol, Protocol::Ssh);
    }

    #[test]
    fn valid_ssh199_server() {
        const DATA: &[u8] = b"SSH-1.99-OpenSSH_7.9p1 Debian-10+deb10u2\n";
        let protocol = check_server(DATA).unwrap();
        assert_eq!(protocol, Protocol::Ssh);
    }

    #[test]
    fn valid_ssh19_legacy_server() {
        const DATA: &[u8] = b"SSH-1.9-OpenSSH_7.9\r\n";
        let protocol = check_server(DATA).unwrap();
        assert_eq!(protocol, Protocol::SshLegacy);
    }

    #[test]
    fn valid_ssh1_legacy_server() {
        const DATA: &[u8] = b"SSH-1.5-OpenSSH_7.9\r\n";
        let protocol = check_server(DATA).unwrap();
        assert_eq!(protocol, Protocol::SshLegacy);
    }

    #[test]
    fn insufficient_data() {
        const DATA: &[u8] = b"SSH-2.";
        let result = check_server(DATA);
        assert!(matches!(result, Err(ProtocolInspectError::NeedMoreData(4))));
    }

    #[test]
    fn invalid_prefix() {
        let mut data = vec![0; 200];
        data[0..8].copy_from_slice(b"XSH-2.0-");
        data[198..200].copy_from_slice(b"\r\n");
        let protocol = check_server(&data).unwrap();
        assert_eq!(protocol, Protocol::Unknown);
    }

    #[test]
    fn invalid_version() {
        let data = b"SSH-3.0-OpenSSH_8.9p1\r\n";
        let protocol = check_server(data).unwrap();
        assert_eq!(protocol, Protocol::Unknown);

        let data = b"SSH-2.1-OpenSSH_8.9p1\r\n";
        let protocol = check_server(data).unwrap();
        assert_eq!(protocol, Protocol::Unknown);

        let data = b"SSH-1.X-OpenSSH_8.9p1\r\n";
        let protocol = check_server(data).unwrap();
        assert_eq!(protocol, Protocol::Unknown);
    }

    #[test]
    fn missing_hyphen_after_version() {
        const DATA: &[u8] = b"SSH-2.0XOpenSSH_8.9p1\r\n";
        let protocol = check_server(DATA).unwrap();
        assert_eq!(protocol, Protocol::Unknown);
    }

    #[test]
    fn missing_lfcr() {
        let data = b"SSH-2.0-OpenSSH_7.9p1 Debian-10+deb10u2";
        let result = check_server(data);
        assert!(matches!(result, Err(ProtocolInspectError::NeedMoreData(1))));

        let data = b"SSH-2.0-OpenSSH_7.9p1 Debian-10+deb10u2\n";
        let protocol = check_server(data).unwrap();
        assert_eq!(protocol, Protocol::Unknown);
    }

    #[test]
    fn max_length_no_newline() {
        let mut data = vec![0; 255];
        data[0..4].copy_from_slice(b"SSH-");
        data[4..8].copy_from_slice(b"2.0-");
        let protocol = check_server(&data).unwrap();
        assert_eq!(protocol, Protocol::Unknown);
    }
}
