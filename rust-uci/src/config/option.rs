use std::{
    ffi::{CStr, CString},
    option::Option as StdOption,
};

use libuci_sys::{
    uci_add_list, uci_foreach_element, uci_option_type_UCI_TYPE_LIST,
    uci_option_type_UCI_TYPE_STRING, uci_set,
};

use crate::{config::handle_error, error::Error, libuci_locked, Result};

use super::ptr::{UciPtr, PTR_STAGE_OPTION};

/// represents an option within a [Section]
pub struct Option {
    ptr: UciPtr<PTR_STAGE_OPTION>,
}

impl Option {
    pub(crate) fn new(ptr: UciPtr<PTR_STAGE_OPTION>) -> Option {
        Option { ptr }
    }

    /// name of the option
    pub fn name(&self) -> Result<&str> {
        Ok(unsafe { CStr::from_ptr((*self.ptr).option) }.to_str()?)
    }

    /// sets the value of the option, overriding the previous value
    /// will create the [Package] or [Section] along the way if they do
    /// not exist
    pub fn set(&self, value: impl AsRef<str>) -> Result<()> {
        let value = CString::new(value.as_ref())?;

        // avoid modifying the long-lived uci_ptr
        let mut ptr = self.ptr.clone();
        ptr.value = value.as_ptr();

        let mut uci = self.ptr.uci.lock().unwrap();
        let result = libuci_locked!(uci, { unsafe { uci_set(uci.ctx, &mut ptr) } });
        handle_error(&mut uci, result)?;

        Ok(())
    }

    /// adds a value to the existing value
    /// behaves like `uci add_list` which will:
    /// - create the option if it doesn't exist (not as a list)
    /// - turn a single-value option into a list
    ///
    /// returns the resulting value
    pub fn add_list<'a>(&'a self, value: impl AsRef<str>) -> Result<()> {
        let value = CString::new(value.as_ref())?;

        // avoid modifying the existing uci_ptr
        let mut ptr = self.ptr.clone();
        ptr.value = value.as_ptr();

        let mut uci = self.ptr.uci.lock().unwrap();
        let result = libuci_locked!(uci, { unsafe { uci_add_list(uci.ctx, &mut ptr) } });
        handle_error(&mut uci, result)?;

        Ok(())
    }

    /// returns the current value of the option, None if not set
    pub fn get<'a>(&'a self) -> Result<StdOption<Value>> {
        let ptr = match self.ptr.lookup()? {
            Some(ptr) => ptr,
            None => return Ok(None),
        };

        let opt = ptr.o;

        #[allow(non_upper_case_globals)]
        match unsafe { *opt }.type_ {
            uci_option_type_UCI_TYPE_STRING => {
                let raw = unsafe { CStr::from_ptr((*opt).v.string) };
                Ok(Value::String(raw.to_str()?.into()))
            }
            uci_option_type_UCI_TYPE_LIST => {
                let mut result = Vec::new();
                unsafe {
                    uci_foreach_element(&(*opt).v.list, |elem| {
                        let raw = CStr::from_ptr((*elem).name);
                        result.push(raw);
                    })
                };
                Ok(Value::List(
                    result
                        .into_iter()
                        .map(|cstr| cstr.to_str().map_err(Into::into).map(Into::into))
                        .collect::<Result<Vec<_>>>()?,
                ))
            }
            t => return Err(Error::Message(format!("Unexpected option type: {t}"))),
        }
        .map(Some)
    }
}

/// represents the value of an [Option]
#[derive(Debug)]
pub enum Value {
    String(String),
    Boolean(bool),
    Integer(i64),
    List(Vec<String>),
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::String(value.into())
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl<'a> PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::String(l0), Self::String(r0)) => l0 == r0,
            (Self::Boolean(l0), Self::Boolean(r0)) => l0 == r0,
            (Self::Integer(l0), Self::Integer(r0)) => l0 == r0,
            (Self::List(l0), Self::List(r0)) => l0 == r0,
            _ => false,
        }
    }
}
