use std::ffi::CStr;

use libuci_sys::{uci_element, uci_package, uci_to_package, uci_to_section};

use crate::{config::lookup_child, Result, Uci};

use super::{section::Section, FromUciElement, NameOrRef};

#[derive(Clone)]
pub struct PackageRef {
    pkg: *const uci_package,
}

impl From<*const uci_package> for PackageRef {
    fn from(pkg: *const uci_package) -> Self {
        Self { pkg }
    }
}

impl PackageRef {
    pub fn get<'a>(self, uci: &'a mut Uci) -> Package<'a> {
        Package {
            uci,
            pkg: NameOrRef::Ref(self),
        }
    }

    pub fn name(&self) -> Result<&str> {
        unsafe { CStr::from_ptr((*self.pkg).path) }
            .to_str()
            .map_err(Into::into)
    }
}

/// represents a single package in the config tree
/// parent to different [Section]s
pub struct Package<'a> {
    uci: &'a mut Uci,
    pkg: NameOrRef<String, PackageRef>,
}

impl<'a> Package<'a> {
    pub(crate) fn new(uci: &'a mut Uci, pkg: NameOrRef<String, PackageRef>) -> Self {
        Self { uci, pkg }
    }

    /// name of this package
    pub fn name(&self) -> Result<&str> {
        match &self.pkg {
            NameOrRef::Name(name) => Ok(name),
            NameOrRef::Ref(pkg) => pkg.name(),
        }
    }

    /// return a single [Section] by its name
    /// also works if the section is not defined yet
    pub fn section(
        &'a mut self,
        name: impl AsRef<str>,
        section_type: impl AsRef<str>,
    ) -> Result<Section<'a>> {
        let name = name.as_ref();
        let section_type = section_type.as_ref();

        let pkg = match &self.pkg {
            NameOrRef::Name(_) => {
                return Ok(Section::new(
                    self.uci,
                    self.pkg.clone(),
                    NameOrRef::Name((name.to_owned(), section_type.to_owned())),
                ))
            }
            NameOrRef::Ref(pkg) => pkg,
        };

        let section = lookup_child(self.uci, &mut unsafe { *pkg.pkg }.sections, name)?
            .map(|ptr| NameOrRef::Ref(unsafe { uci_to_section(ptr) }.into()))
            .unwrap_or_else(|| NameOrRef::Name((name.to_owned(), section_type.to_owned())));
        Ok(Section::new(self.uci, self.pkg.clone(), section))
    }

    /// list all [Section]s in this package
    pub fn sections(&'a mut self) -> Result<impl Iterator<Item = Section<'a>>> {
        Ok(vec![].into_iter())
    }
}

unsafe impl FromUciElement for PackageRef {
    /// # Safety
    /// - Caller must guarantee that elem contains a `uci_package`
    /// - Caller must guarantee that elem is not null
    unsafe fn from_uci_element(elem: *const uci_element) -> Self {
        Self {
            pkg: uci_to_package(elem),
        }
    }
}
