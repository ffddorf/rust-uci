use std::{ffi::CStr, marker::PhantomData, ptr};

use libuci_sys::{uci_package, uci_to_section};

use crate::{Result, Uci};

use super::{
    lookup_child,
    section::{Section, SectionRef},
    NameOrRef, UciListIter,
};

#[derive(Clone)]
pub struct PackageRef<'a> {
    pkg: *const uci_package,
    /// lifetime of UCI context where we got pkg from
    _lt: &'a PhantomData<()>,
}

impl<'a> PackageRef<'a> {
    pub(crate) unsafe fn new(pkg: *const uci_package) -> Self {
        Self {
            pkg,
            _lt: &PhantomData,
        }
    }

    pub fn get(self, uci: &'a mut Uci) -> Package<'a> {
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
    pkg: NameOrRef<String, PackageRef<'a>>,
}

impl<'a> Package<'a> {
    pub(crate) fn new(uci: &'a mut Uci, pkg: NameOrRef<String, PackageRef<'a>>) -> Self {
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
        section_type: impl AsRef<str>,
        name: impl AsRef<str>,
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
            .map(|ptr| NameOrRef::Ref(unsafe { SectionRef::new(pkg.pkg, uci_to_section(ptr)) }))
            .unwrap_or_else(|| NameOrRef::Name((name.to_owned(), section_type.to_owned())));
        Ok(Section::new(self.uci, self.pkg.clone(), section))
    }

    /// list all [Section]s in this package
    pub fn sections(&'a mut self) -> impl Iterator<Item = SectionRef<'a>> {
        let (pkg, list) = match &self.pkg {
            NameOrRef::Name(_) => (ptr::null(), ptr::null()),
            NameOrRef::Ref(pkg) => (pkg.pkg, &unsafe { *pkg.pkg }.sections as *const _),
        };
        UciListIter::new(list)
            .map(move |elem| unsafe { SectionRef::new(pkg, uci_to_section(elem)) })
    }
}
