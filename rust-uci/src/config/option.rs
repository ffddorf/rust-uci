use std::option::Option as StdOption;

use crate::{Result, UciPtr};

/// represents the value of an [Option]
pub enum Value {
    String(String),
    Boolean(bool),
    Integer(i64),
    List(Vec<Value>),
}

/// represents an option within a [Section]
pub struct Option {
    _ptr: UciPtr,
}

impl Option {
    /// name of the option
    pub fn name(&mut self) -> Result<String> {
        todo!()
    }

    /// returns the current value of the option, None if not set
    pub fn get(&mut self) -> Result<StdOption<Value>> {
        todo!()
    }

    /// sets the value of the option, overriding the previous value
    /// will create the [Package] or [Section] along the way if they do
    /// not exist
    pub fn set(&mut self, _value: Value) -> Result<()> {
        todo!()
    }

    /// adds a value to the existing value
    /// behaves like `uci add_list` which will:
    /// - create the option if it doesn't exist (not as a list)
    /// - turn a single-value option into a list
    ///
    /// returns the resulting value
    pub fn add_list(&mut self, _value: Value) -> Result<Value> {
        todo!()
    }
}
