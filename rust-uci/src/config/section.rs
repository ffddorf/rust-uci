use std::{
    ffi::{CStr, CString},
    option::Option as StdOption,
    sync::Arc,
};

use libuci_sys::uci_set;

use crate::{config::handle_error, libuci_locked, Result};

use super::{
    option::Option,
    ptr::{UciListIter, UciPtr, PTR_STAGE_SECTION},
};

/// represents a single section
/// parent to different [Option]s
pub struct Section {
    ptr: UciPtr<PTR_STAGE_SECTION>,
}

impl Section {
    pub(crate) fn new(ptr: UciPtr<PTR_STAGE_SECTION>) -> Self {
        Self { ptr }
    }

    pub fn create(&self, type_: impl AsRef<str>) -> Result<()> {
        let type_ = CString::new(type_.as_ref())?;

        // avoid modifying the long-lived uci_ptr
        let mut ptr = self.ptr.clone();
        ptr.value = type_.as_ptr();

        let uci = Arc::clone(&self.ptr.uci);
        let mut uci = uci.lock().unwrap();
        let result = libuci_locked!(uci, unsafe { uci_set(uci.ctx, &mut ptr) });
        handle_error(&mut uci, result)?;

        Ok(())
    }

    /// returns the name of the section item
    pub fn name(&self) -> Result<&str> {
        Ok(unsafe { CStr::from_ptr(self.ptr.section) }.to_str()?)
    }

    /// returns the type of the section, if it exists
    pub fn type_(&self) -> Result<StdOption<String>> {
        let ptr = match self.ptr.lookup()? {
            Some(ptr) => ptr,
            None => return Ok(None),
        };
        Ok(Some(
            unsafe { CStr::from_ptr((*ptr.s).type_) }
                .to_str()?
                .to_owned(),
        ))
    }

    /// lists all options in this section
    pub fn options(&self) -> impl Iterator<Item = Option> + use<'_> {
        let iter = unsafe { UciListIter::new(&(*self.ptr.s).options) };
        iter.map(move |elem| {
            let name = unsafe { CStr::from_ptr((*elem).name) }.to_str().unwrap();
            Option::new(self.ptr.with_option_name(name).unwrap())
        })
    }

    /// returns a specific [Option] by name
    /// also works if the option is not defined yet
    pub fn option(&self, name: impl AsRef<str>) -> Result<Option> {
        let ptr = self.ptr.with_option_name(name)?;
        Ok(Option::new(ptr))
    }
}
