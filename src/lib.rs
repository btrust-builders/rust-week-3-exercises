use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Deref;
use hex;

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
        CompactSize { value }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let v = self.value;
        if v <= 0xFC {
            vec![v as u8]
        } else if v <= 0xFFFF {
            let mut out = vec![0xFD];
            out.extend_from_slice(&(v as u16).to_le_bytes());
            out
        } else if v <= 0xFFFF_FFFF {
            let mut out = vec![0xFE];
            out.extend_from_slice(&(v as u32).to_le_bytes());
            out
        } else {
            let mut out = vec![0xFF];
            out.extend_from_slice(&v.to_le_bytes());
            out
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.is_empty() {
            return Err(BitcoinError::InsufficientBytes);
        }
        match bytes[0] {
            n @ 0x00..=0xFC => Ok((CompactSize::new(n as u64), 1)),
            0xFD => {
                if bytes.len() < 3 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let mut arr = [0u8; 2];
                arr.copy_from_slice(&bytes[1..3]);
                Ok((CompactSize::new(u16::from_le_bytes(arr) as u64), 3))
            }
            0xFE => {
                if bytes.len() < 5 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let mut arr = [0u8; 4];
                arr.copy_from_slice(&bytes[1..5]);
                Ok((CompactSize::new(u32::from_le_bytes(arr) as u64), 5))
            }
            0xFF => {
                if bytes.len() < 9 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let mut arr = [0u8; 8];
                arr.copy_from_slice(&bytes[1..9]);
                Ok((CompactSize::new(u64::from_le_bytes(arr)), 9))
            }
            _ => Err(BitcoinError::InvalidFormat),
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
        serializer.serialize_str(&hex::encode(self.0))
    }
}

impl<'de> Deserialize<'de> for Txid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let bytes = hex::decode(&s).map_err(serde::de::Error::custom)?;
        if bytes.len() != 32 {
            return Err(serde::de::Error::custom("Txid must be 32 bytes"));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Txid(arr))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct OutPoint {
    pub txid: Txid,
    pub vout: u32,
}

impl OutPoint {
    pub fn new(txid: [u8; 32], vout: u32) -> Self {
        OutPoint {
            txid: Txid(txid),
            vout,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(36);
        out.extend_from_slice(&self.txid.0);
        out.extend_from_slice(&self.vout.to_le_bytes());
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.len() < 36 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let mut txid = [0u8; 32];
        txid.copy_from_slice(&bytes[0..32]);
        let mut vout_bytes = [0u8; 4];
        vout_bytes.copy_from_slice(&bytes[32..36]);
        let vout = u32::from_le_bytes(vout_bytes);
        Ok((OutPoint::new(txid, vout), 36))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Script {
    pub bytes: Vec<u8>,
}

impl Script {
    pub fn new(bytes: Vec<u8>) -> Self {
        Script { bytes }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = CompactSize::new(self.bytes.len() as u64).to_bytes();
        out.extend_from_slice(&self.bytes);
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let (len_cs, cs_len) = CompactSize::from_bytes(bytes)?;
        let total_len = cs_len + (len_cs.value as usize);
        if bytes.len() < total_len {
            return Err(BitcoinError::InsufficientBytes);
        }
        let script_bytes = bytes[cs_len..total_len].to_vec();
        Ok((Script::new(script_bytes), total_len))
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
        TransactionInput {
            previous_output,
            script_sig,
            sequence,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = self.previous_output.to_bytes();
        out.extend(self.script_sig.to_bytes());
        out.extend_from_slice(&self.sequence.to_le_bytes());
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let (outpoint, outpoint_len) = OutPoint::from_bytes(bytes)?;
        let (script, script_len) = Script::from_bytes(&bytes[outpoint_len..])?;
        let seq_start = outpoint_len + script_len;
        if bytes.len() < seq_start + 4 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let mut seq_bytes = [0u8; 4];
        seq_bytes.copy_from_slice(&bytes[seq_start..seq_start + 4]);
        let sequence = u32::from_le_bytes(seq_bytes);
        Ok((TransactionInput::new(outpoint, script, sequence), seq_start + 4))
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
        BitcoinTransaction {
            version,
            inputs,
            lock_time,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = self.version.to_le_bytes().to_vec();
        out.extend(CompactSize::new(self.inputs.len() as u64).to_bytes());
        for input in &self.inputs {
            out.extend(input.to_bytes());
        }
        out.extend_from_slice(&self.lock_time.to_le_bytes());
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.len() < 4 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let mut version_bytes = [0u8; 4];
        version_bytes.copy_from_slice(&bytes[0..4]);
        let version = u32::from_le_bytes(version_bytes);
        let (cs, cs_len) = CompactSize::from_bytes(&bytes[4..])?;
        let mut offset = 4 + cs_len;
        let mut inputs = Vec::with_capacity(cs.value as usize);
        for _ in 0..cs.value {
            let (input, consumed) = TransactionInput::from_bytes(&bytes[offset..])?;
            inputs.push(input);
            offset += consumed;
        }
        if bytes.len() < offset + 4 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let mut lock_time_bytes = [0u8; 4];
        lock_time_bytes.copy_from_slice(&bytes[offset..offset + 4]);
        let lock_time = u32::from_le_bytes(lock_time_bytes);
        Ok((BitcoinTransaction::new(version, inputs, lock_time), offset + 4))
    }
}

impl fmt::Display for BitcoinTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Version: {}", self.version)?;
        writeln!(f, "Lock Time: {}", self.lock_time)?;
        for (i, input) in self.inputs.iter().enumerate() {
            writeln!(f, "Input {}:", i)?;
            writeln!(f, "  Previous Output Txid: {}", hex::encode(input.previous_output.txid.0))?;
            writeln!(f, "  Previous Output Vout: {}", input.previous_output.vout)?;
            writeln!(f, "  ScriptSig ({} bytes): {}", input.script_sig.bytes.len(), hex::encode(&input.script_sig.bytes))?;
            writeln!(f, "  Sequence: {}", input.sequence)?;
        }
        Ok(())
    }
}
