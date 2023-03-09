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

use anyhow::anyhow;
use humanize_rs::bytes::Bytes;
use yaml_rust::Yaml;

pub fn as_usize(v: &Yaml) -> anyhow::Result<usize> {
    match v {
        Yaml::String(value) => {
            let v = value.parse::<Bytes>()?;
            Ok(v.size())
        }
        Yaml::Integer(value) => Ok(usize::try_from(*value)?),
        _ => Err(anyhow!(
            "yaml value type for humanize usize should be 'string' or 'integer'"
        )),
    }
}

pub fn as_u32(v: &Yaml) -> anyhow::Result<u32> {
    match v {
        Yaml::String(value) => {
            let v = value.parse::<Bytes>()?;
            let v = u32::try_from(v.size()).map_err(|e| anyhow!("invalid u32 value: {e}"))?;
            Ok(v)
        }
        Yaml::Integer(value) => Ok(u32::try_from(*value)?),
        _ => Err(anyhow!(
            "yaml value type for humanize usize should be 'string' or 'integer'"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_usize() {
        let v = Yaml::String("1000".to_string());
        assert_eq!(as_usize(&v).unwrap(), 1000);

        let v = Yaml::String("1K".to_string());
        assert_eq!(as_usize(&v).unwrap(), 1000);

        let v = Yaml::String("1KB".to_string());
        assert_eq!(as_usize(&v).unwrap(), 1000);

        let v = Yaml::String("1KiB".to_string());
        assert_eq!(as_usize(&v).unwrap(), 1024);

        let v = Yaml::Integer(1024);
        assert_eq!(as_usize(&v).unwrap(), 1024);

        let v = Yaml::Integer(-1024);
        assert!(as_usize(&v).is_err());

        let v = Yaml::Real("1.01".to_string());
        assert!(as_usize(&v).is_err());

        let v = Yaml::Array(vec![Yaml::Integer(1)]);
        assert!(as_usize(&v).is_err());
    }
}
