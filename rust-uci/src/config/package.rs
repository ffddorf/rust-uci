use std::{ffi::CStr, sync::Arc};

use crate::Result;

use super::{
    ptr::{UciListIter, UciPtr, PTR_STAGE_PACKAGE},
    section::Section,
};

/// represents a single package in the config tree
/// parent to different [Section]s
// todo: get rid of const generic, use enum
pub struct Package {
    ptr: UciPtr<PTR_STAGE_PACKAGE>,
}

impl Package {
    pub(crate) fn new(ptr: UciPtr<PTR_STAGE_PACKAGE>) -> Package {
        Package { ptr }
    }

    pub fn name(&self) -> Result<&str> {
        Ok(unsafe { CStr::from_ptr(self.ptr.package) }.to_str()?)
    }

    pub fn sections(&self) -> Result<impl Iterator<Item = Section> + use<'_>> {
        let ptr = match self.ptr.lookup()? {
            Some(ptr) => ptr,
            None => todo!(),
        };
        let iter = unsafe { UciListIter::new(&(*ptr.p).sections) };
        let ptr_inner = Arc::new(ptr);
        Ok(iter.map(move |elem| {
            let name = unsafe { CStr::from_ptr((*elem).name) }.to_str().unwrap();
            Section::new(ptr_inner.with_section_name(name).unwrap())
        }))
    }

    /// return a single [Section] by its name
    /// also works if the section is not defined yet
    pub fn section(&self, name: impl AsRef<str>) -> Result<Section> {
        let ptr = self.ptr.with_section_name(name)?;
        Ok(Section::new(ptr))
    }
}
