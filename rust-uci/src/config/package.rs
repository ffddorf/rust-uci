use std::{ffi::CStr, sync::Arc};

use libuci_sys::uci_to_section;

use crate::Result;

use super::{
    ptr::{UciListIter, UciPtr, PTR_STAGE_PACKAGE},
    section::Section,
};

/// represents a single package in the config tree
/// parent to different [Section]s
// todo: get rid of const generic, use enum
pub struct Package<const L: bool> {
    ptr: UciPtr<PTR_STAGE_PACKAGE, L>,
}

impl Package<false> {
    pub(crate) fn new<const L: bool>(ptr: UciPtr<PTR_STAGE_PACKAGE, L>) -> Package<L> {
        Package { ptr }
    }

    pub fn name(&self) -> Result<&str> {
        Ok(unsafe { CStr::from_ptr(self.ptr.package) }.to_str()?)
    }

    pub fn sections(&self) -> Result<impl Iterator<Item = Section<true>> + use<'_>> {
        let ptr = match self.ptr.lookup()? {
            Some(ptr) => ptr,
            None => todo!(),
        };
        let iter = unsafe { UciListIter::new(&(*ptr.p).sections) };
        let ptr_inner = Arc::new(ptr);
        Ok(iter.map(move |elem| {
            Section::new(ptr_inner.with_section(unsafe { uci_to_section(elem).cast_mut() }))
        }))
    }
}

impl Package<true> {
    /// name of this package
    pub fn name(&self) -> Result<&str> {
        self.ptr.name()
    }

    /// list all [Section]s in this package
    pub fn sections(&self) -> impl Iterator<Item = Section<true>> + use<'_> {
        let iter = unsafe { UciListIter::new(&(*self.ptr.p).sections) };
        iter.map(move |elem| {
            Section::new(
                self.ptr
                    .with_section(unsafe { uci_to_section(elem).cast_mut() }),
            )
        })
    }
}

impl<const L: bool> Package<L> {
    /// return a single [Section] by its name
    /// also works if the section is not defined yet
    pub fn section(
        &self,
        _type_: impl AsRef<str>,
        name: impl AsRef<str>,
    ) -> Result<Section<false>> {
        // todo: constrain lookup by section type
        // let type_ = type_.as_ref();

        let ptr = self.ptr.with_section_name(name)?;
        Ok(Section::new(ptr))
    }
}
