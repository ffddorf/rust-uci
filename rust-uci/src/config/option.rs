use std::{
    borrow::Cow,
    ffi::{CStr, CString},
    option::Option as StdOption,
    sync::{Arc, Mutex},
};

use libuci_sys::{
    uci_add_list, uci_foreach_element, uci_option_type_UCI_TYPE_LIST,
    uci_option_type_UCI_TYPE_STRING, uci_set,
};

use crate::{config::handle_error, error::Error, libuci_locked, Result, Uci};

use super::ptr::{UciPtr, PTR_STAGE_OPTION};

/// represents an option within a [Section]
pub struct Option<const L: bool> {
    uci: Arc<Mutex<Uci>>,
    ptr: UciPtr<PTR_STAGE_OPTION, L>,
}

impl<const L: bool> Option<L> {
    fn get_impl<'a>(ptr: &UciPtr<PTR_STAGE_OPTION, true>) -> Result<Value<'a>> {
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
    }

    /// sets the value of the option, overriding the previous value
    /// will create the [Package] or [Section] along the way if they do
    /// not exist
    pub fn set(&mut self, value: impl AsRef<str>) -> Result<Option<true>> {
        let value = CString::new(value.as_ref())?;
        let mut ptr = self.ptr.clone();
        ptr.value = value.as_ptr();

        let mut uci = self.uci.lock().unwrap();
        let result = libuci_locked!(uci, { unsafe { uci_set(uci.ctx, &mut ptr) } });
        handle_error(&mut uci, result)?;

        Ok(Option::new(
            Arc::clone(&self.uci),
            self.ptr.with_update(ptr),
        ))
    }

    /// adds a value to the existing value
    /// behaves like `uci add_list` which will:
    /// - create the option if it doesn't exist (not as a list)
    /// - turn a single-value option into a list
    ///
    /// returns the resulting value
    pub fn add_list<'a>(&'a self, value: impl AsRef<str>) -> Result<Option<true>> {
        let value = CString::new(value.as_ref())?;
        let mut ptr = self.ptr.clone();
        ptr.value = value.as_ptr();

        let mut uci = self.uci.lock().unwrap();
        let result = libuci_locked!(uci, { unsafe { uci_add_list(uci.ctx, &mut ptr) } });
        handle_error(&mut uci, result)?;

        Ok(Option::new(
            Arc::clone(&self.uci),
            self.ptr.with_update(ptr),
        ))
    }
}

impl Option<false> {
    pub(crate) fn new<const L: bool>(
        uci: Arc<Mutex<Uci>>,
        ptr: UciPtr<PTR_STAGE_OPTION, L>,
    ) -> Option<L> {
        Option { uci, ptr }
    }

    /// name of the option
    pub fn name(&self) -> Result<&str> {
        Ok(unsafe { CStr::from_ptr((*self.ptr).option) }.to_str()?)
    }

    /// returns the current value of the option, None if not set
    pub fn get<'a>(&'a self) -> Result<StdOption<Value<'a>>> {
        let mut uci = self.uci.lock().unwrap();
        let ptr = match self.ptr.lookup(&mut uci)? {
            Some(ptr) => ptr,
            None => return Ok(None),
        };
        Self::get_impl(&ptr).map(Some)
    }
}

impl Option<true> {
    /// name of the option
    pub fn name(&self) -> Result<&str> {
        self.ptr.name()
    }

    /// returns the current value of the option, None if not set
    pub fn get<'a>(&'a self) -> Result<Value<'a>> {
        Self::get_impl(&self.ptr)
    }
}

/// represents the value of an [Option]
#[derive(Debug)]
pub enum Value<'a> {
    String(Cow<'a, str>),
    Boolean(bool),
    Integer(i64),
    List(Vec<Cow<'a, str>>),
}

impl<'a> Value<'a> {
    pub fn to_static(self) -> Value<'static> {
        match self {
            Value::String(cow) => Value::String(cow.into_owned().into()),
            Value::Boolean(v) => Value::Boolean(v),
            Value::Integer(v) => Value::Integer(v),
            Value::List(values) => {
                Value::List(values.into_iter().map(|v| v.into_owned().into()).collect())
            }
        }
    }
}

impl From<String> for Value<'static> {
    fn from(value: String) -> Self {
        Self::String(value.into())
    }
}

impl From<bool> for Value<'static> {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}

impl From<i64> for Value<'static> {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl<'a> PartialEq for Value<'a> {
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
