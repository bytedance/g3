/*
 * Copyright 2023 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::convert::TryFrom;
use std::str::FromStr;
use std::time::Duration;

use anyhow::anyhow;
use humanize_rs::ParseError;
use yaml_rust::Yaml;

pub fn as_duration(v: &Yaml) -> anyhow::Result<Duration> {
    match v {
        Yaml::String(value) => match humanize_rs::duration::parse(value) {
            Ok(v) => Ok(v),
            Err(ParseError::MissingUnit) => {
                if let Ok(u) = u64::from_str(value) {
                    Ok(Duration::from_secs(u))
                } else if let Ok(f) = f64::from_str(value) {
                    Duration::try_from_secs_f64(f).map_err(anyhow::Error::new)
                } else {
                    Err(anyhow!("invalid duration string"))
                }
            }
            Err(e) => Err(anyhow!("invalid humanize duration string: {e}")),
        },
        Yaml::Integer(value) => {
            if let Ok(u) = u64::try_from(*value) {
                Ok(Duration::from_secs(u))
            } else {
                Err(anyhow!("unsupported duration string"))
            }
        }
        Yaml::Real(s) => {
            let f = f64::from_str(s).map_err(|e| anyhow!("invalid f64 value: {e}"))?;
            Duration::try_from_secs_f64(f).map_err(anyhow::Error::new)
        }
        _ => Err(anyhow!(
            "yaml value type for humanize duration should be 'string' or 'integer' or 'real'"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_duration() {
        let v = Yaml::String("1h2m".to_string());
        assert_eq!(as_duration(&v).unwrap(), Duration::from_secs(3600 + 120));

        let v = Yaml::String("1000".to_string());
        assert_eq!(as_duration(&v).unwrap(), Duration::from_secs(1000));

        let v = Yaml::String("-1000".to_string());
        assert!(as_duration(&v).is_err());

        let v = Yaml::String("1.01".to_string());
        assert!(as_duration(&v).is_err());

        let v = Yaml::String("-1000h".to_string());
        assert!(as_duration(&v).is_err());

        let v = Yaml::String("1000Ah".to_string());
        assert!(as_duration(&v).is_err());

        let v = Yaml::Integer(1000);
        assert_eq!(as_duration(&v).unwrap(), Duration::from_secs(1000));

        let v = Yaml::Integer(-1000);
        assert!(as_duration(&v).is_err());

        let v = Yaml::Real("1.01".to_string());
        assert_eq!(
            as_duration(&v).unwrap(),
            Duration::try_from_secs_f64(1.01).unwrap()
        );

        let v = Yaml::Array(vec![Yaml::Integer(1)]);
        assert!(as_duration(&v).is_err());
    }
}
