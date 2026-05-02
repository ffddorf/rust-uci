use std::{
    ffi::{CStr, CString},
    option::Option as StdOption,
    ptr,
    sync::{Arc, Mutex},
};

use libuci_sys::{uci_commit, uci_save, uci_section, uci_to_section, uci_type_UCI_TYPE_PACKAGE};

use crate::{
    config::{handle_error, section::SectionIdent},
    error::Error,
    libuci_locked, Result, Uci,
};

use super::{
    ptr::{UciListIter, UciPtr},
    section::Section,
};

/// represents a single package in the config tree
/// parent to different [Section]s
pub struct Package {
    uci: Arc<Mutex<Uci>>,
    name: Arc<CString>,
}

impl Package {
    pub(crate) fn new(uci: Arc<Mutex<Uci>>, name: Arc<CString>) -> Package {
        Package { uci, name }
    }

    pub fn name(&self) -> Result<&str> {
        Ok(self.name.to_str()?)
    }

    pub(crate) fn ptr_opt<'a>(&'_ self, uci: &'a mut Uci) -> Result<StdOption<UciPtr<'a>>> {
        let mut ptr = UciPtr::new();
        ptr.target = uci_type_UCI_TYPE_PACKAGE;
        ptr.package = self.name.as_c_str().as_ptr();
        ptr.lookup(uci)
    }

    pub(crate) fn ptr<'a>(&'_ self, uci: &'a mut Uci) -> Result<UciPtr<'a>> {
        match self.ptr_opt(uci)? {
            Some(ptr) => Ok(ptr),
            None => Err(Error::EntryNotFound {
                entry_identifier: self.name()?.to_owned(),
            }),
        }
    }

    pub(crate) unsafe fn section_ident(section: *const uci_section) -> SectionIdent<CString> {
        let elem = &unsafe { *section }.e;
        match unsafe { *section }.anonymous {
            true => UciListIter::new(&unsafe { *(*section).package }.sections)
                .enumerate()
                .find(|(_, sect_elem)| *sect_elem == elem)
                .map(|(i, _)| SectionIdent::Indexed(i as i32))
                .unwrap_or(SectionIdent::Anonymous),
            false => {
                let name = unsafe { CStr::from_ptr((*elem).name) }.to_owned();
                SectionIdent::Named(name)
            }
        }
    }

    pub fn sections(&self) -> Result<impl Iterator<Item = Section>> {
        let mut uci = self.uci.lock().unwrap();
        let ptr = match self.ptr_opt(&mut uci)? {
            Some(ptr) => unsafe { &(*ptr.p).sections },
            None => ptr::null(),
        };
        drop(uci);
        let uci = Arc::clone(&self.uci);
        let package = Arc::clone(&self.name);
        Ok(UciListIter::new(ptr).map(move |elem| {
            let sect = unsafe { uci_to_section(elem) };
            let type_ = unsafe { CStr::from_ptr((*sect).type_) }.to_owned();
            let ident = unsafe { Self::section_ident(sect) };
            Section::new(
                Arc::clone(&uci),
                Arc::clone(&package),
                Arc::new(type_),
                Arc::new(ident),
            )
        }))
    }

    /// return a single [Section] by its name
    /// also works if the section is not defined yet
    pub fn section(
        &self,
        type_: impl AsRef<str>,
        ident: SectionIdent<impl AsRef<str>>,
    ) -> Result<Section> {
        let type_ = CString::new(type_.as_ref())?;

        use SectionIdent::*;
        let ident = match ident {
            Anonymous => Anonymous,
            Indexed(i) => Indexed(i),
            Named(n) => Named(CString::new(n.as_ref())?),
        };

        Ok(Section::new(
            Arc::clone(&self.uci),
            Arc::clone(&self.name),
            Arc::new(type_),
            Arc::new(ident),
        ))
    }

    /// save package delta to disk
    pub fn save(&mut self) -> Result<()> {
        let mut uci = self.uci.lock().unwrap();
        let pkg = self.ptr(&mut uci)?.p;
        let result = libuci_locked!(uci, unsafe { uci_save(uci.ctx, pkg) });
        handle_error(&mut uci, result)?;
        Ok(())
    }

    /// commit package delta into real config on disk
    pub fn commit(&mut self) -> Result<()> {
        let mut uci = self.uci.lock().unwrap();
        let mut pkg = self.ptr(&mut uci)?.p;
        // the uci cli seems to set `override=false` too, not sure what it means
        let result = libuci_locked!(uci, unsafe { uci_commit(uci.ctx, &raw mut pkg, false) });
        handle_error(&mut uci, result)?;
        Ok(())
    }
}
