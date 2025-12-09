//! Fusabi bytecode format and loader.
//!
//! The Fusabi bytecode format (.fzb) is a simple binary format for
//! representing compiled plugin code.
//!
//! ## Format
//!
//! ```text
//! +----------------+
//! | Magic (4 bytes)|  "FZB\x01" (version 1)
//! +----------------+
//! | Header         |
//! +----------------+
//! | Constant Pool  |
//! +----------------+
//! | Instructions   |
//! +----------------+
//! | Debug Info     |  (optional)
//! +----------------+
//! ```
//!
//! For the initial implementation, we use a JSON-based intermediate
//! representation that can be executed by the runtime.

use crate::error::{RuntimeError, RuntimeResult};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Magic bytes for Fusabi bytecode files.
pub const MAGIC: &[u8; 4] = b"FZB\x01";

/// Fusabi bytecode representation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bytecode {
    /// Version of the bytecode format.
    pub version: u8,

    /// Plugin metadata embedded in bytecode.
    pub metadata: BytecodeMetadata,

    /// Constant pool.
    pub constants: Vec<Constant>,

    /// Function definitions.
    pub functions: Vec<Function>,

    /// Entry point function name.
    pub entry_point: String,
}

/// Metadata embedded in bytecode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BytecodeMetadata {
    /// Plugin ID.
    pub plugin_id: String,

    /// Plugin version.
    pub plugin_version: String,

    /// Compilation timestamp.
    pub compiled_at: Option<String>,

    /// Compiler version.
    pub compiler_version: Option<String>,
}

/// A constant value in the constant pool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum Constant {
    /// Null value.
    Null,
    /// Boolean value.
    Bool(bool),
    /// Integer value.
    Int(i64),
    /// Float value.
    Float(f64),
    /// String value.
    String(String),
}

/// A function definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    /// Function name.
    pub name: String,

    /// Parameter names.
    pub params: Vec<String>,

    /// Instructions.
    pub instructions: Vec<Instruction>,

    /// Local variable count.
    pub local_count: usize,
}

/// A bytecode instruction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum Instruction {
    /// Load a constant from the pool.
    LoadConst { index: usize },

    /// Load a local variable.
    LoadLocal { index: usize },

    /// Store to a local variable.
    StoreLocal { index: usize },

    /// Load a global variable.
    LoadGlobal { name: String },

    /// Store to a global variable.
    StoreGlobal { name: String },

    /// Call a function.
    Call { name: String, arg_count: usize },

    /// Call a method on an object.
    CallMethod { name: String, arg_count: usize },

    /// Return from function.
    Return,

    /// Jump to offset.
    Jump { offset: i32 },

    /// Jump if top of stack is false.
    JumpIfFalse { offset: i32 },

    /// Pop value from stack.
    Pop,

    /// Duplicate top of stack.
    Dup,

    /// Binary add.
    Add,

    /// Binary subtract.
    Sub,

    /// Binary multiply.
    Mul,

    /// Binary divide.
    Div,

    /// Comparison: equal.
    Eq,

    /// Comparison: not equal.
    Ne,

    /// Comparison: less than.
    Lt,

    /// Comparison: less than or equal.
    Le,

    /// Comparison: greater than.
    Gt,

    /// Comparison: greater than or equal.
    Ge,

    /// Logical not.
    Not,

    /// Logical and.
    And,

    /// Logical or.
    Or,

    /// Create array from N items on stack.
    MakeArray { count: usize },

    /// Create object from N key-value pairs on stack.
    MakeObject { count: usize },

    /// Get property from object.
    GetProperty { name: String },

    /// Set property on object.
    SetProperty { name: String },

    /// Get index from array/object.
    GetIndex,

    /// Set index in array/object.
    SetIndex,

    /// Await an async value.
    Await,

    /// No operation.
    Nop,
}

/// Bytecode loader.
pub struct BytecodeLoader;

impl BytecodeLoader {
    /// Load bytecode from a file.
    pub fn load(path: &Path) -> RuntimeResult<Bytecode> {
        let content = std::fs::read(path)?;
        Self::parse(&content)
    }

    /// Parse bytecode from bytes.
    pub fn parse(bytes: &[u8]) -> RuntimeResult<Bytecode> {
        // Check for magic bytes
        if bytes.len() < 4 {
            return Err(RuntimeError::BytecodeError(
                "File too small to be valid bytecode".to_string(),
            ));
        }

        if &bytes[0..4] == MAGIC {
            // Binary format - parse header and content
            Self::parse_binary(&bytes[4..])
        } else {
            // Try JSON format (development/debug format)
            Self::parse_json(bytes)
        }
    }

    /// Parse binary bytecode format.
    fn parse_binary(bytes: &[u8]) -> RuntimeResult<Bytecode> {
        // For now, binary format after magic is JSON
        // Future: implement proper binary parsing
        Self::parse_json(bytes)
    }

    /// Parse JSON bytecode format.
    fn parse_json(bytes: &[u8]) -> RuntimeResult<Bytecode> {
        let content = std::str::from_utf8(bytes)
            .map_err(|e| RuntimeError::BytecodeError(format!("Invalid UTF-8: {}", e)))?;

        serde_json::from_str(content)
            .map_err(|e| RuntimeError::BytecodeError(format!("Invalid bytecode JSON: {}", e)))
    }

    /// Validate bytecode structure.
    pub fn validate(bytecode: &Bytecode) -> RuntimeResult<()> {
        // Check version
        if bytecode.version != 1 {
            return Err(RuntimeError::BytecodeError(format!(
                "Unsupported bytecode version: {}",
                bytecode.version
            )));
        }

        // Check entry point exists
        let has_entry = bytecode
            .functions
            .iter()
            .any(|f| f.name == bytecode.entry_point);

        if !has_entry {
            return Err(RuntimeError::BytecodeError(format!(
                "Entry point function '{}' not found",
                bytecode.entry_point
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_bytecode() -> Bytecode {
        Bytecode {
            version: 1,
            metadata: BytecodeMetadata {
                plugin_id: "test".to_string(),
                plugin_version: "0.1.0".to_string(),
                compiled_at: None,
                compiler_version: None,
            },
            constants: vec![
                Constant::String("Hello".to_string()),
                Constant::Int(42),
            ],
            functions: vec![Function {
                name: "main".to_string(),
                params: vec![],
                instructions: vec![
                    Instruction::LoadConst { index: 0 },
                    Instruction::Return,
                ],
                local_count: 0,
            }],
            entry_point: "main".to_string(),
        }
    }

    #[test]
    fn test_serialize_bytecode() {
        let bc = sample_bytecode();
        let json = serde_json::to_string_pretty(&bc).unwrap();
        assert!(json.contains("\"version\": 1"));
        assert!(json.contains("\"entry_point\": \"main\""));
    }

    #[test]
    fn test_parse_json_bytecode() {
        let bc = sample_bytecode();
        let json = serde_json::to_vec(&bc).unwrap();
        let parsed = BytecodeLoader::parse(&json).unwrap();
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.entry_point, "main");
    }

    #[test]
    fn test_validate_bytecode() {
        let bc = sample_bytecode();
        assert!(BytecodeLoader::validate(&bc).is_ok());
    }

    #[test]
    fn test_validate_missing_entry_point() {
        let mut bc = sample_bytecode();
        bc.entry_point = "nonexistent".to_string();
        assert!(BytecodeLoader::validate(&bc).is_err());
    }
}
