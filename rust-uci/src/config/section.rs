use std::{ffi::CStr, marker::PhantomData, ptr};

use libuci_sys::{uci_package, uci_section, uci_to_option};

use crate::{Result, Uci};

use super::{
    lookup_child,
    option::{Option, OptionRef},
    package::PackageRef,
    NameOrRef, UciListIter,
};

pub struct SectionRef<'a> {
    pkg: *const uci_package,
    section: *const uci_section,
    _lt: &'a PhantomData<()>,
}

impl<'a> SectionRef<'a> {
    pub(crate) unsafe fn new(pkg: *const uci_package, section: *const uci_section) -> Self {
        Self {
            pkg,
            section,
            _lt: &PhantomData,
        }
    }

    pub fn get(self, uci: &'a mut Uci) -> Section<'a> {
        Section::new(
            uci,
            NameOrRef::Ref(unsafe { PackageRef::new(self.pkg) }),
            NameOrRef::Ref(self),
        )
    }

    pub fn name(&self) -> Result<&str> {
        let ptr = unsafe { *self.section }.e.name;
        Ok(unsafe { CStr::from_ptr(ptr) }.to_str()?)
    }

    pub fn r#type(&self) -> Result<&str> {
        let ptr = unsafe { *self.section }.type_;
        Ok(unsafe { CStr::from_ptr(ptr) }.to_str()?)
    }
}

/// represents a single section
/// parent to different [Option]s
pub struct Section<'a> {
    uci: &'a mut Uci,
    parent: NameOrRef<String, PackageRef<'a>>,
    section: NameOrRef<(String, String), SectionRef<'a>>,
}

impl<'a> Section<'a> {
    pub(crate) fn new(
        uci: &'a mut Uci,
        parent: NameOrRef<String, PackageRef<'a>>,
        section: NameOrRef<(String, String), SectionRef<'a>>,
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
            NameOrRef::Ref(section) => section.name(),
        }
    }

    /// returns the type of the section
    pub fn r#type(&mut self) -> Result<&str> {
        match &self.section {
            NameOrRef::Name((_, stype)) => Ok(stype),
            NameOrRef::Ref(section) => section.r#type(),
        }
    }

    /// returns a specific [Option] by name
    /// also works if the option is not defined yet
    pub fn option(&'a mut self, name: impl AsRef<str>) -> Result<Option<'a>> {
        let name = name.as_ref();
        let section = match &self.section {
            NameOrRef::Name(_name) => {
                return Ok(Option::new(self.uci, NameOrRef::Name(name.to_owned())))
            }
            NameOrRef::Ref(section) => section,
        };

        let option = lookup_child(self.uci, &mut unsafe { *section.section }.options, name)?
            .map(|elem| unsafe { NameOrRef::Ref(OptionRef::new(uci_to_option(elem))) })
            .unwrap_or_else(|| NameOrRef::Name(name.to_owned()));
        Ok(Option::new(self.uci, option))
    }

    /// lists all options in this section
    pub fn options(&'a mut self) -> impl Iterator<Item = OptionRef<'a>> {
        let (_pkg, list) = match &self.section {
            NameOrRef::Name(_) => (ptr::null(), ptr::null()),
            NameOrRef::Ref(section) => (
                section.section,
                &unsafe { *section.section }.options as *const _,
            ),
        };
        UciListIter::new(list).map(move |elem| unsafe { OptionRef::new(uci_to_option(elem)) })
    }
}
