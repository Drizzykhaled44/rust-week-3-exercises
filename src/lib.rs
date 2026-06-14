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
        CompactSize { value }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        if self.value <= 0xFC {
            bytes.push(self.value as u8);
        } else if self.value <= 0xFFFF {
            bytes.push(0xFD);
            bytes.extend_from_slice(&(self.value as u16).to_le_bytes());
        } else if self.value <= 0xFFFFFFFF {
            bytes.push(0xFE);
            bytes.extend_from_slice(&(self.value as u32).to_le_bytes());
        } else {
            bytes.push(0xFF);
            bytes.extend_from_slice(&self.value.to_le_bytes());
        }
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.is_empty() {
            return Err(BitcoinError::InsufficientBytes);
        }
        let prefix = bytes[0];
        match prefix {
            0x00..=0xFC => Ok((CompactSize::new(prefix as u64), 1)),
            0xFD => {
                if bytes.len() < 3 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let value = u16::from_le_bytes([bytes[1], bytes[2]]) as u64;
                Ok((CompactSize::new(value), 3))
            }
            0xFE => {
                if bytes.len() < 5 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let value = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as u64;
                Ok((CompactSize::new(value), 5))
            }
            0xFF => {
                if bytes.len() < 9 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let value = u64::from_le_bytes([
                    bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7], bytes[8],
                ]);
                Ok((CompactSize::new(value), 9))
            }
            // all u8 values are covered above; no other prefixes are valid
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
        let hex = hex::encode(self.0);
        serializer.serialize_str(&hex)
    }
}

impl<'de> Deserialize<'de> for Txid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let hex_str = String::deserialize(deserializer)?;
        let bytes = hex::decode(&hex_str).map_err(serde::de::Error::custom)?;
        if bytes.len() != 32 {
            return Err(serde::de::Error::custom("Txid must be exactly 32 bytes"));
        }
        let mut txid_bytes = [0u8; 32];
        txid_bytes.copy_from_slice(&bytes);
        Ok(Txid(txid_bytes))
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
        let mut bytes = self.txid.0.to_vec();
        bytes.extend_from_slice(&self.vout.to_le_bytes());
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.len() < 36 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let mut txid_bytes = [0u8; 32];
        txid_bytes.copy_from_slice(&bytes[0..32]);
        let vout = u32::from_le_bytes([bytes[32], bytes[33], bytes[34], bytes[35]]);
        Ok((
            OutPoint {
                txid: Txid(txid_bytes),
                vout,
            },
            36,
        ))
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
        let mut bytes = CompactSize::new(self.bytes.len() as u64).to_bytes();
        bytes.extend_from_slice(&self.bytes);
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let (size, consumed) = CompactSize::from_bytes(bytes)?;
        let total_len = consumed + size.value as usize;
        if bytes.len() < total_len {
            return Err(BitcoinError::InsufficientBytes);
        }
        let script_bytes = bytes[consumed..total_len].to_vec();
        Ok((
            Script {
                bytes: script_bytes,
            },
            total_len,
        ))
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
        let mut bytes = self.previous_output.to_bytes();
        bytes.extend_from_slice(&self.script_sig.to_bytes());
        bytes.extend_from_slice(&self.sequence.to_le_bytes());
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let (outpoint, offset1) = OutPoint::from_bytes(bytes)?;
        let remaining = &bytes[offset1..];
        let (script, offset2) = Script::from_bytes(remaining)?;
        let total_so_far = offset1 + offset2;
        if bytes.len() < total_so_far + 4 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let sequence = u32::from_le_bytes([
            bytes[total_so_far],
            bytes[total_so_far + 1],
            bytes[total_so_far + 2],
            bytes[total_so_far + 3],
        ]);
        Ok((
            TransactionInput {
                previous_output: outpoint,
                script_sig: script,
                sequence,
            },
            total_so_far + 4,
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
        BitcoinTransaction {
            version,
            inputs,
            lock_time,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = self.version.to_le_bytes().to_vec();
        let num_inputs = CompactSize::new(self.inputs.len() as u64);
        bytes.extend_from_slice(&num_inputs.to_bytes());
        for input in &self.inputs {
            bytes.extend_from_slice(&input.to_bytes());
        }
        bytes.extend_from_slice(&self.lock_time.to_le_bytes());
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.len() < 4 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let version = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let (num_inputs_cs, offset1) = CompactSize::from_bytes(&bytes[4..])?;
        let mut total_consumed = 4 + offset1;
        let mut inputs = Vec::with_capacity(num_inputs_cs.value as usize);
        for _ in 0..num_inputs_cs.value {
            if total_consumed >= bytes.len() {
                return Err(BitcoinError::InsufficientBytes);
            }
            let (input, consumed) = TransactionInput::from_bytes(&bytes[total_consumed..])?;
            inputs.push(input);
            total_consumed += consumed;
        }
        if bytes.len() < total_consumed + 4 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let lock_time = u32::from_le_bytes([
            bytes[total_consumed],
            bytes[total_consumed + 1],
            bytes[total_consumed + 2],
            bytes[total_consumed + 3],
        ]);
        let total = total_consumed + 4;
        Ok((
            BitcoinTransaction {
                version,
                inputs,
                lock_time,
            },
            total,
        ))
    }
}

impl fmt::Display for BitcoinTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Bitcoin Transaction:")?;
        writeln!(f, " Version: {}", self.version)?;
        writeln!(f, " Input: {} (count)", self.inputs.len())?;
        for (i, input) in self.inputs.iter().enumerate() {
            writeln!(f, " Input #{}:", i)?;
            writeln!(f, " Previous Output:")?;
            writeln!(f, "  Txid: {}", hex::encode(input.previous_output.txid.0))?;
            writeln!(f, "  Vout: {}", input.previous_output.vout)?;
            writeln!(f, " Previous Output Vout: {}", input.previous_output.vout)?;
            writeln!(f, " ScriptSig:")?;
            writeln!(f, "  Length: {} bytes", input.script_sig.len())?;
            writeln!(f, "  Bytes: {}", hex::encode(&input.script_sig.bytes))?;
            writeln!(f, " Sequence: 0x{:08x}", input.sequence)?;
        }
        writeln!(f, " Lock Time: {}", self.lock_time)?;
        Ok(())
    }
}
