use std::{
    ffi::CStr,
    sync::{Arc, Mutex},
};

use libuci_sys::uci_to_option;

use crate::{Result, Uci};

use super::{
    option::Option,
    ptr::{UciPtr, PTR_STAGE_SECTION},
};

/// represents a single section
/// parent to different [Option]s
pub struct Section<const L: bool> {
    uci: Arc<Mutex<Uci>>,
    ptr: UciPtr<PTR_STAGE_SECTION, L>,
}

impl<const L: bool> Section<L> {
    pub(crate) fn new(uci: Arc<Mutex<Uci>>, ptr: UciPtr<PTR_STAGE_SECTION, L>) -> Self {
        Self { uci, ptr }
    }
}

impl Section<true> {
    /// returns the name of named sections, otherwise None
    pub fn name(&self) -> Result<&str> {
        self.ptr.name()
    }

    /// returns the type of the section
    pub fn type_(&mut self) -> Result<&str> {
        Ok(unsafe { CStr::from_ptr((*self.ptr.s).type_) }.to_str()?)
    }

    /// lists all options in this section
    pub fn options(&self) -> impl Iterator<Item = Option<true>> + use<'_> {
        let uci = Arc::clone(&self.uci);
        self.ptr.children().map(move |elem| {
            Option::new(
                Arc::clone(&uci),
                self.ptr
                    .with_option(unsafe { uci_to_option(elem).cast_mut() }),
            )
        })
    }
}

impl<const L: bool> Section<L> {
    /// returns a specific [Option] by name
    /// also works if the option is not defined yet
    pub fn option(&self, name: impl AsRef<str>) -> Result<Option<false>> {
        let ptr = self.ptr.with_option_name(name)?;
        Ok(Option::new(Arc::clone(&self.uci), ptr))
    }
}
