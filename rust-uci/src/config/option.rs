use std::{
    ffi::{CStr, CString},
    ops::DerefMut,
    option::Option as StdOption,
    sync::{Arc, Mutex},
};

use libuci_sys::{
    uci_add_list, uci_foreach_element, uci_option_type_UCI_TYPE_LIST,
    uci_option_type_UCI_TYPE_STRING, uci_set, uci_type_UCI_TYPE_OPTION,
};

use crate::{config::handle_error, error::Error, libuci_locked, Result, Uci};

use super::{
    ptr::UciPtr,
    section::{Section, SectionIdent},
};

/// represents an option within a [Section]
pub struct Option {
    uci: Arc<Mutex<Uci>>,
    package: Arc<CString>,
    section: (Arc<CString>, Arc<SectionIdent<CString>>),
    name: Arc<CString>,
}

impl Option {
    pub(crate) fn new(
        uci: Arc<Mutex<Uci>>,
        package: Arc<CString>,
        section: (Arc<CString>, Arc<SectionIdent<CString>>),
        name: Arc<CString>,
    ) -> Option {
        Option {
            uci,
            package,
            section,
            name,
        }
    }

    fn ptr<'a>(&'_ self, uci: &'a mut Uci) -> Result<StdOption<UciPtr<'a>>> {
        let section = match self.section().ptr(uci)? {
            Some(s) => s,
            None => return Ok(None),
        };

        let mut ptr = UciPtr::new();
        ptr.target = uci_type_UCI_TYPE_OPTION;
        ptr.p = section.p;
        ptr.s = section.s;
        ptr.option = self.name.as_ptr();
        ptr.lookup(uci)
    }

    fn ptr_ensure<'a>(&'a mut self) -> Result<UciPtr<'a>> {
        let mut uci = self.uci.lock().unwrap();
        let mut section = self.section();
        let section_ptr = section.ensure(Some(&mut uci))?;

        // update ident to match newly created item
        self.section.1 = Arc::clone(&section.ident);

        let mut ptr = UciPtr::new();
        ptr.target = uci_type_UCI_TYPE_OPTION;
        ptr.p = section_ptr.p;
        ptr.s = section_ptr.s;
        ptr.option = self.name.as_ptr();
        Ok(ptr)
    }

    /// name of the option
    pub fn name(&self) -> &str {
        self.name.to_str().unwrap()
    }

    pub fn section(&self) -> Section {
        Section::new(
            Arc::clone(&self.uci),
            Arc::clone(&self.package),
            Arc::clone(&self.section.0),
            Arc::clone(&self.section.1),
        )
    }

    /// sets the value of the option, overriding the previous value
    /// will create the [Package] or [Section] along the way if they do
    /// not exist
    pub fn set(&mut self, value: impl AsRef<str>) -> Result<()> {
        let value = CString::new(value.as_ref())?;

        let mut ptr = self.ptr_ensure()?;
        ptr.value = value.as_ptr();
        let ptr = ptr.deref_mut() as *mut _;

        let mut uci = self.uci.lock().unwrap();
        let result = libuci_locked!(uci, { unsafe { uci_set(uci.ctx, ptr) } });
        handle_error(&mut uci, result)?;

        Ok(())
    }

    /// adds a value to the existing value
    /// behaves like `uci add_list` which will:
    /// - create the option if it doesn't exist (not as a list)
    /// - turn a single-value option into a list
    ///
    /// returns the resulting value
    pub fn add_list(&mut self, value: impl AsRef<str>) -> Result<()> {
        let value = CString::new(value.as_ref())?;

        let mut ptr = self.ptr_ensure()?;
        ptr.value = value.as_ptr();
        let ptr = ptr.deref_mut() as *mut _;

        let mut uci = self.uci.lock().unwrap();
        let result = libuci_locked!(uci, { unsafe { uci_add_list(uci.ctx, ptr) } });
        handle_error(&mut uci, result)?;

        Ok(())
    }

    /// returns the current value of the option, None if not set
    pub fn get<'a>(&'a self) -> Result<StdOption<Value>> {
        let mut uci = self.uci.lock().unwrap();
        let ptr = match self.ptr(&mut uci)? {
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
