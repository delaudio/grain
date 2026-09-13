//! Small, validated numeric inputs shared by creative engines.

use std::collections::BTreeMap;
use std::fmt;

use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

use super::RuntimeDiagnostic;

pub const MAX_PARAMETERS: usize = 64;
pub const MAX_PARAMETER_MAGNITUDE: f64 = 1_000_000.0;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Parameters(BTreeMap<String, f64>);

impl Parameters {
    pub fn get(&self, name: &str) -> Option<f64> {
        self.0.get(name).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, f64)> {
        self.0.iter().map(|(name, value)| (name.as_str(), *value))
    }

    pub fn set(&mut self, name: &str, value: f64) -> Result<(), RuntimeDiagnostic> {
        let invalid = |message: &str| RuntimeDiagnostic {
            message: message.into(),
            line: None,
            column: None,
            stack: None,
        };
        if name.is_empty()
            || name.len() > 64
            || !name.bytes().enumerate().all(|(index, byte)| {
                byte == b'_' || byte.is_ascii_alphabetic() || (index > 0 && byte.is_ascii_digit())
            })
            || matches!(name, "__proto__" | "constructor" | "prototype")
        {
            return Err(invalid(
                "Parameter names must be 1-64 ASCII identifier characters, excluding reserved prototype names",
            ));
        }
        if !value.is_finite() || value.abs() > MAX_PARAMETER_MAGNITUDE {
            return Err(invalid(
                "Parameter values must be finite numbers between -1000000 and 1000000",
            ));
        }
        if !self.0.contains_key(name) && self.0.len() >= MAX_PARAMETERS {
            return Err(invalid("A sketch can have at most 64 parameters"));
        }
        self.0.insert(name.into(), value);
        Ok(())
    }

    pub fn remove(&mut self, name: &str) -> Option<f64> {
        self.0.remove(name)
    }
}

impl<'de> Deserialize<'de> for Parameters {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ParameterVisitor;
        impl<'de> Visitor<'de> for ParameterVisitor {
            type Value = Parameters;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an object of at most 64 named finite numeric parameters")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
                let mut result = Parameters::default();
                while let Some((name, value)) = map.next_entry::<String, f64>()? {
                    if result.0.contains_key(&name) {
                        return Err(serde::de::Error::custom("Duplicate parameter name"));
                    }
                    result.set(&name, value).map_err(serde::de::Error::custom)?;
                }
                Ok(result)
            }
        }
        deserializer.deserialize_map(ParameterVisitor)
    }
}
