use std::ffi::CStr;

use libuci_sys::uci_to_option;

use crate::Result;

use super::{
    option::Option,
    ptr::{UciListIter, UciPtr, PTR_STAGE_SECTION},
};

/// represents a single section
/// parent to different [Option]s
// todo: get rid of const generic, use enum
pub struct Section<const L: bool> {
    ptr: UciPtr<PTR_STAGE_SECTION, L>,
}

impl<const L: bool> Section<L> {
    pub(crate) fn new(ptr: UciPtr<PTR_STAGE_SECTION, L>) -> Self {
        Self { ptr }
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
        let iter = unsafe { UciListIter::new(&(*self.ptr.s).options) };
        iter.map(move |elem| {
            Option::new(
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
        Ok(Option::new(ptr))
    }
}
