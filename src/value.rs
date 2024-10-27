use anyhow::Error;
use bytes::BufMut;
use serde_json::{self, json};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    String(Vec<u8>),
    Integer(isize),
    Array(Vec<Value>),
    Dict(BTreeMap<Vec<u8>, Value>),
}

impl Value {
    pub fn encode(&self) -> Vec<u8> {
        match self {
            Value::String(s) => {
                let mut len = s.len();
                let mut str = vec![];
                loop {
                    if len < 10 {
                        str.push(len as u8 + b'0');
                        break;
                    }
                    let n = len % 10;
                    len /= 10;
                    str.push(n as u8 + b'0')
                }
                str.reverse();

                let mut string = vec![];
                string.put(&str[..]);
                string.put(&b":"[..]);
                string.put(&s[..]);
                string
            }
            Value::Integer(mut i) => {
                // -52
                let is_negative = i.is_negative();
                if i.is_negative() {
                    i *= -1;
                }
                let mut integer = vec![];
                loop {
                    if i < 10 {
                        integer.push(i as u8 + b'0');
                        break;
                    }
                    let n = i % 10;
                    i /= 10;
                    integer.push(n as u8 + b'0')
                }
                if is_negative {
                    integer.push(b'-');
                }
                integer.push(b'i');
                integer.reverse();
                integer.push(b'e');
                integer
            }
            Value::Array(array) => {
                let mut encoded_array = vec![];
                encoded_array.push(b'l');
                for value in array {
                    let mut v = value.encode();
                    encoded_array.append(&mut v);
                }
                encoded_array.push(b'e');
                encoded_array
            }
            Value::Dict(dict) => {
                let mut encoded_dict = vec![];
                encoded_dict.push(b'd');
                for (key, value) in dict {
                    let mut key = Value::String(key.clone()).encode();
                    let mut value = value.encode();
                    encoded_dict.append(&mut key);
                    encoded_dict.append(&mut value);
                }
                encoded_dict.push(b'e');
                encoded_dict
            }
        }
    }

    pub fn decode(encoded_value: &[u8]) -> anyhow::Result<Self> {
        let (value, rest) = decode_bencoded_value(encoded_value)?;
        if !rest.is_empty() {
            return Err(anyhow::Error::msg(
                "some bytes cannot be docoded into value",
            ));
        }

        Ok(value)
    }

    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Value::String(s) => {
                json!(s.iter().map(|c| *c as char).collect::<String>())
            }
            Value::Integer(i) => json!(i),
            Value::Array(a) => json!(a
                .iter()
                .map(|ai| ai.to_json())
                .collect::<Vec<serde_json::Value>>()),
            Value::Dict(d) => {
                json!(d.iter().fold(serde_json::Map::new(), |mut acc, n| {
                    acc.insert(
                        n.0.iter().map(|c| *c as char).collect::<String>(),
                        n.1.to_json(),
                    );
                    acc
                }))
            }
        }
    }
}

pub fn decode_string(mut encoded_value: &[u8]) -> anyhow::Result<(Value, &[u8])> {
    let mut len: usize = 0;
    let mut index = 0;

    while encoded_value[index].is_ascii_digit() && encoded_value[index] != b':' {
        let value = encoded_value[index] - b'0';
        len *= 10;
        len += value as usize;
        index += 1;
    }
    // len /= 10;
    index += 1; // skip b':'
    encoded_value = &encoded_value[index..];

    let string = encoded_value[..len].to_vec();
    Ok((Value::String(string), &encoded_value[len..]))
}

pub fn decode_integer(encoded_value: &[u8]) -> anyhow::Result<(Value, &[u8])> {
    if encoded_value[0] == b'i' {
        // b'i'
        let mut len = 1;
        let integer = encoded_value[1..]
            .iter()
            .take_while(|c| {
                len += 1;
                **c != b'e'
            })
            .map(|c| *c as char)
            .collect::<String>()
            .parse::<isize>()
            .unwrap();

        let encoded_value = &encoded_value[len..];

        return Ok((Value::Integer(integer), encoded_value));
    }

    Err(Error::msg("Failed decoding Integer"))
}

fn decode_list(encoded_value: &[u8]) -> anyhow::Result<(Value, &[u8])> {
    if encoded_value[0] == b'l' {
        let mut values = vec![];
        let mut rest = &encoded_value[1..];
        loop {
            if rest[0] == b'e' {
                break;
            }
            let (value, rest_inner) = decode_bencoded_value(rest)?;
            values.push(value);
            rest = rest_inner;
        }

        return Ok((Value::Array(values), &rest[1..]));
    }
    Err(Error::msg("Failed decoding List"))
}

pub fn decode_dict(encoded_value: &[u8]) -> anyhow::Result<(Value, &[u8])> {
    if encoded_value[0] == b'd' {
        let mut values = BTreeMap::new();
        let mut rest = &encoded_value[1..];
        loop {
            if rest[0] == b'e' {
                break;
            }
            let (key, rest_inner) = decode_string(rest)?;
            let (value, rest_inner) = decode_bencoded_value(rest_inner)?;
            // NOTE: key must be string in bencoded value
            if let Value::String(s) = key {
                values.insert(s, value);
            }
            rest = rest_inner;
        }

        return Ok((Value::Dict(values), &rest[1..]));
    }
    Err(Error::msg("Failed decoding Dict"))
}

pub fn decode_bencoded_value(encoded_value: &[u8]) -> anyhow::Result<(Value, &[u8])> {
    match encoded_value[0] {
        b'i' => decode_integer(encoded_value),
        b'l' => decode_list(encoded_value),
        b'd' => decode_dict(encoded_value),
        c if c.is_ascii_digit() => decode_string(encoded_value),
        _ => Err(anyhow::Error::msg("cannot parse value")),
    }
}
