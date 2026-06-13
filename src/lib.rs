use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Deref;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct CompactSize {
    pub value: u64,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BitcoinError {
    InsufficientBytes,
    InvalidFormat,
}

impl CompactSize {
    pub fn new(value: u64) -> Self {
        Self { value }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        if self.value <= 0xFC {
            vec![self.value as u8]
        } else if self.value <= 0xFFFF {
            let mut result = vec![0xFD];
            result.extend((self.value as u16).to_le_bytes());
            result
        } else if self.value <= 0xFFFF_FFFF {
            let mut result = vec![0xFE];
            result.extend((self.value as u32).to_le_bytes());
            result
        } else {
            let mut result = vec![0xFF];
            result.extend(self.value.to_le_bytes());
            result
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.is_empty() {
            return Err(BitcoinError::InsufficientBytes);
        }

        let prefix = bytes[0];

        if prefix <= 0xFC {
            Ok((Self::new(prefix as u64), 1))
        } else if prefix == 0xFD {
            if bytes.len() < 3 {
                return Err(BitcoinError::InsufficientBytes);
            }

            let value = u16::from_le_bytes([bytes[1], bytes[2]]);

            Ok((Self::new(value as u64), 3))
        } else if prefix == 0xFE {
            if bytes.len() < 5 {
                return Err(BitcoinError::InsufficientBytes);
            }

            let value = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);

            Ok((Self::new(value as u64), 5))
        } else {
            if bytes.len() < 9 {
                return Err(BitcoinError::InsufficientBytes);
            }

            let value = u64::from_le_bytes([
                bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7], bytes[8],
            ]);

            Ok((Self::new(value), 9))
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Txid(pub [u8; 32]);

impl Serialize for Txid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let hex_string = hex::encode(self.0);
        serializer.serialize_str(&hex_string)
    }
}

impl<'de> Deserialize<'de> for Txid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let hex_string = String::deserialize(deserializer)?;

        let bytes = hex::decode(&hex_string).map_err(serde::de::Error::custom)?;

        if bytes.len() != 32 {
            return Err(serde::de::Error::custom("txid must be exactly 32 bytes"));
        }

        let array: [u8; 32] = bytes
            .try_into()
            .map_err(|_| serde::de::Error::custom("invalid txid length"))?;

        Ok(Txid(array))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct OutPoint {
    pub txid: Txid,
    pub vout: u32,
}

impl OutPoint {
    pub fn new(txid: [u8; 32], vout: u32) -> Self {
        Self {
            txid: Txid(txid),
            vout,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = Vec::new();

        result.extend(self.txid.0);

        result.extend(self.vout.to_le_bytes());

        result
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.len() < 36 {
            return Err(BitcoinError::InsufficientBytes);
        }

        let txid_bytes: [u8; 32] = bytes[0..32]
            .try_into()
            .map_err(|_| BitcoinError::InvalidFormat)?;

        let txid = Txid(txid_bytes);

        let vout = u32::from_le_bytes(
            bytes[32..36]
                .try_into()
                .map_err(|_| BitcoinError::InvalidFormat)?,
        );

        Ok((Self { txid, vout }, 36))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Script {
    pub bytes: Vec<u8>,
}

impl Script {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = CompactSize::new(self.bytes.len() as u64).to_bytes();

        result.extend(&self.bytes);

        result
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let (length, length_size) = CompactSize::from_bytes(bytes)?;

        let script_length = length.value as usize;

        if bytes.len() < length_size + script_length {
            return Err(BitcoinError::InsufficientBytes);
        }

        let script_bytes = bytes[length_size..length_size + script_length].to_vec();

        Ok((Script::new(script_bytes), length_size + script_length))
    }
}

impl Deref for Script {
    type Target = Vec<u8>;
    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct TransactionInput {
    pub previous_output: OutPoint,
    pub script_sig: Script,
    pub sequence: u32,
}

impl TransactionInput {
    pub fn new(previous_output: OutPoint, script_sig: Script, sequence: u32) -> Self {
        Self {
            previous_output,
            script_sig,
            sequence,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = Vec::new();

        result.extend(self.previous_output.to_bytes());

        result.extend(self.script_sig.to_bytes());

        result.extend(self.sequence.to_le_bytes());

        result
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let (previous_output, consumed_outpoint) = OutPoint::from_bytes(bytes)?;

        let mut offset = consumed_outpoint;

        let (script_sig, consumed_script) = Script::from_bytes(&bytes[offset..])?;

        offset += consumed_script;

        if bytes.len() < offset + 4 {
            return Err(BitcoinError::InsufficientBytes);
        }

        let sequence = u32::from_le_bytes(
            bytes[offset..offset + 4]
                .try_into()
                .map_err(|_| BitcoinError::InvalidFormat)?,
        );

        offset += 4;

        Ok((
            TransactionInput::new(previous_output, script_sig, sequence),
            offset,
        ))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct BitcoinTransaction {
    pub version: u32,
    pub inputs: Vec<TransactionInput>,
    pub lock_time: u32,
}

impl BitcoinTransaction {
    pub fn new(version: u32, inputs: Vec<TransactionInput>, lock_time: u32) -> Self {
        Self {
            version,
            inputs,
            lock_time,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = Vec::new();

        result.extend(self.version.to_le_bytes());

        result.extend(CompactSize::new(self.inputs.len() as u64).to_bytes());

        for input in &self.inputs {
            result.extend(input.to_bytes());
        }

        result.extend(self.lock_time.to_le_bytes());

        result
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
           if bytes.len() < 4 {
        return Err(BitcoinError::InsufficientBytes);
    }

    let version = u32::from_le_bytes(
        bytes[0..4]
            .try_into()
            .map_err(|_| BitcoinError::InvalidFormat)?
    );

    let mut offset = 4;

    let (input_count, count_size) =
        CompactSize::from_bytes(&bytes[offset..])?;

    offset += count_size;

    let mut inputs = Vec::new();

    for _ in 0..input_count.value {
        let (input, consumed) =
            TransactionInput::from_bytes(
                &bytes[offset..]
            )?;

        inputs.push(input);

        offset += consumed;
    }

    if bytes.len() < offset + 4 {
        return Err(BitcoinError::InsufficientBytes);
    }

    let lock_time = u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .map_err(|_| BitcoinError::InvalidFormat)?
    );

    offset += 4;

    Ok((
        BitcoinTransaction::new(
            version,
            inputs,
            lock_time,
        ),
        offset,
    ))
    }
}

impl fmt::Display for BitcoinTransaction {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        writeln!(f, "BitcoinTransaction")?;
        writeln!(f, "Version: {}", self.version)?;
        writeln!(f, "Inputs: {}", self.inputs.len())?;

        for (index, input) in self.inputs.iter().enumerate() {
            writeln!(f, "Input {}:", index)?;

            writeln!(
                f,
                "Previous Output TXID: {}",
                hex::encode(input.previous_output.txid.0)
            )?;

            writeln!(
                f,
                "Previous Output Vout: {}",
                input.previous_output.vout
            )?;

            writeln!(
                f,
                "ScriptSig Length: {}",
                input.script_sig.bytes.len()
            )?;

            writeln!(
                f,
                "ScriptSig: {}",
                hex::encode(&input.script_sig.bytes)
            )?;

            writeln!(
                f,
                "Sequence: {}",
                input.sequence
            )?;
        }

        writeln!(f, "Lock Time: {}", self.lock_time)?;

        Ok(())
    }
}

