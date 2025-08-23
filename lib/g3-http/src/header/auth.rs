/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use base64::prelude::*;

use g3_types::auth::{Password, Username};

pub fn proxy_authorization_basic(username: &Username, password: &Password) -> String {
    format!(
        "Proxy-Authorization: Basic {}\r\n",
        BASE64_STANDARD.encode(format!(
            "{}:{}",
            username.as_original(),
            password.as_original()
        ))
    )
}

pub fn proxy_authenticate_basic(realm: &str) -> String {
    format!("Proxy-Authenticate: Basic realm=\"{realm}\"\r\n")
}

pub fn www_authenticate_basic(realm: &str) -> String {
    format!("WWW-Authenticate: Basic realm=\"{realm}\"\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_proxy_authorization_basic() {
        // Normal case
        let username = Username::from_original("user").unwrap();
        let password = Password::from_original("pass").unwrap();
        let expected = "Proxy-Authorization: Basic dXNlcjpwYXNz\r\n";
        assert_eq!(proxy_authorization_basic(&username, &password), expected);

        // Empty credentials
        let empty_user = Username::empty();
        let empty_pass = Password::empty();
        let expected_empty = "Proxy-Authorization: Basic Og==\r\n"; // ":" base64 encoded
        assert_eq!(
            proxy_authorization_basic(&empty_user, &empty_pass),
            expected_empty
        );

        // Special characters
        let special_user = Username::from_original("user@domain").unwrap();
        let special_pass = Password::from_original("p@ss:w0rd").unwrap();
        let expected_special = "Proxy-Authorization: Basic dXNlckBkb21haW46cEBzczp3MHJk\r\n";
        assert_eq!(
            proxy_authorization_basic(&special_user, &special_pass),
            expected_special
        );
    }

    #[test]
    fn t_proxy_authenticate_basic() {
        let realm = "test_realm";
        let expected = format!("Proxy-Authenticate: Basic realm=\"{realm}\"\r\n");
        assert_eq!(proxy_authenticate_basic(realm), expected);

        let special_realm = "realm with spaces@!";
        let expected_special = format!("Proxy-Authenticate: Basic realm=\"{}\"\r\n", special_realm);
        assert_eq!(proxy_authenticate_basic(special_realm), expected_special);
    }

    #[test]
    fn t_www_authenticate_basic() {
        let realm = "web_realm";
        let expected = format!("WWW-Authenticate: Basic realm=\"{realm}\"\r\n");
        assert_eq!(www_authenticate_basic(realm), expected);

        let empty_realm = "";
        let expected_empty = "WWW-Authenticate: Basic realm=\"\"\r\n";
        assert_eq!(www_authenticate_basic(empty_realm), expected_empty);
    }
}
