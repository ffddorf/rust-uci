use std::{
    ffi::CStr,
    sync::{Arc, Mutex},
};

use libuci_sys::uci_to_section;

use crate::{Result, Uci};

use super::{
    ptr::{UciPtr, PTR_STAGE_PACKAGE},
    section::Section,
};

/// represents a single package in the config tree
/// parent to different [Section]s
pub struct Package<const L: bool> {
    uci: Arc<Mutex<Uci>>,
    ptr: UciPtr<PTR_STAGE_PACKAGE, L>,
}

impl Package<false> {
    pub(crate) fn new<const L: bool>(
        uci: Arc<Mutex<Uci>>,
        ptr: UciPtr<PTR_STAGE_PACKAGE, L>,
    ) -> Package<L> {
        Package { uci, ptr }
    }

    pub fn name(&self) -> Result<&str> {
        Ok(unsafe { CStr::from_ptr(self.ptr.package) }.to_str()?)
    }
}

impl Package<true> {
    /// name of this package
    pub fn name(&self) -> Result<&str> {
        self.ptr.name()
    }

    /// list all [Section]s in this package
    pub fn sections(&self) -> impl Iterator<Item = Section<true>> + use<'_> {
        let uci = Arc::clone(&self.uci);
        self.ptr.children().map(move |elem| {
            Section::new(
                Arc::clone(&uci),
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
        Ok(Section::new(Arc::clone(&self.uci), ptr))
    }
}
