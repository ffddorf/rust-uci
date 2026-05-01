use std::ffi::CStr;

use libuci_sys::uci_section;

use crate::{
    config::{package::PackageRef, NameOrRef},
    Result, Uci,
};

use super::option::Option;

pub struct SectionRef {
    section: *const uci_section,
}

impl SectionRef {
    pub fn name(&self) -> Result<&str> {
        let ptr = unsafe { *self.section }.e.name;
        Ok(unsafe { CStr::from_ptr(ptr) }.to_str()?)
    }
}

impl From<*const uci_section> for SectionRef {
    fn from(section: *const uci_section) -> Self {
        Self { section }
    }
}

/// represents a single section
/// parent to different [Option]s
pub struct Section<'a> {
    uci: &'a mut Uci,
    parent: NameOrRef<String, PackageRef>,
    section: NameOrRef<(String, String), SectionRef>,
}

impl<'a> Section<'a> {
    pub(crate) fn new(
        uci: &'a mut Uci,
        parent: NameOrRef<String, PackageRef>,
        section: NameOrRef<(String, String), SectionRef>,
    ) -> Self {
        Self {
            uci,
            parent,
            section,
        }
    }

    /// returns the name of named sections, otherwise None
    pub fn name(&mut self) -> Result<&str> {
        match &self.section {
            NameOrRef::Name((name, _)) => Ok(name),
            NameOrRef::Ref(_) => todo!(),
        }
    }

    /// returns the type of the section
    pub fn r#type(&mut self) -> Result<&str> {
        match &self.section {
            NameOrRef::Name((_, stype)) => Ok(stype),
            NameOrRef::Ref(_) => todo!(),
        }
    }

    /// returns a specific [Option] by name
    /// also works if the option is not defined yet
    pub fn option(&mut self, _name: impl AsRef<str>) -> Result<Option> {
        todo!()
    }

    /// lists all options in this section
    pub fn options(&mut self) -> Result<impl Iterator<Item = Option>> {
        Ok(vec![].into_iter())
    }
}
