use std::{
    ffi::{CStr, CString},
    iter::once,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    option::Option as StdOption,
    ptr,
    sync::{Arc, Mutex},
};

use libuci_sys::{
    list_to_element, uci_element, uci_list, uci_lookup_ptr, uci_option, uci_package, uci_ptr,
    uci_ptr_UCI_LOOKUP_COMPLETE, uci_section, uci_type_UCI_TYPE_OPTION, uci_type_UCI_TYPE_PACKAGE,
    uci_type_UCI_TYPE_SECTION, uci_type_UCI_TYPE_UNSPEC,
};

use crate::{config::handle_error, libuci_locked, Result, Uci};

pub(super) struct UciListIter<'a> {
    list: *const uci_list,
    ptr: *const uci_list,
    _lt: &'a PhantomData<()>,
}

impl<'a> UciListIter<'a> {
    /// Safety: list cannot be null
    pub unsafe fn new(list: *const uci_list) -> Self {
        Self {
            list,
            ptr: unsafe { *list }.next,
            _lt: &PhantomData,
        }
    }
}

impl<'a> Iterator for UciListIter<'a> {
    type Item = *const uci_element;

    fn next(&mut self) -> StdOption<Self::Item> {
        if self.ptr.is_null() {
            return None;
        }
        if self.ptr == self.list {
            return None;
        }

        let elem = unsafe { list_to_element(self.ptr) };
        self.ptr = unsafe { *elem }.list.next;

        Some(elem)
    }
}

pub(crate) const PTR_STAGE_INIT: usize = 0;
pub(crate) const PTR_STAGE_PACKAGE: usize = 1;
pub(crate) const PTR_STAGE_SECTION: usize = 2;
pub(crate) const PTR_STAGE_OPTION: usize = 3;

pub(crate) struct UciPtr<const S: usize, const L: bool> {
    pub(crate) uci: Arc<Mutex<Uci>>,
    ptr: uci_ptr,
    data: Vec<Arc<CString>>,
}

impl<const S: usize, const L: bool> Deref for UciPtr<S, L> {
    type Target = uci_ptr;

    fn deref(&self) -> &Self::Target {
        &self.ptr
    }
}

impl<const S: usize, const L: bool> DerefMut for UciPtr<S, L> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ptr
    }
}

impl UciPtr<PTR_STAGE_INIT, false> {
    pub fn new(uci: Arc<Mutex<Uci>>) -> Self {
        let ptr = uci_ptr {
            target: uci_type_UCI_TYPE_UNSPEC,
            flags: 0,
            p: ptr::null_mut(),
            s: ptr::null_mut(),
            o: ptr::null_mut(),
            last: ptr::null_mut(),
            package: ptr::null(),
            section: ptr::null(),
            option: ptr::null(),
            value: ptr::null(),
        };
        Self {
            uci,
            ptr,
            data: Vec::new(),
        }
    }

    pub fn with_package(&self, pkg: *mut uci_package) -> UciPtr<PTR_STAGE_PACKAGE, true> {
        let mut ptr = self.ptr.clone();
        ptr.target = uci_type_UCI_TYPE_PACKAGE;
        ptr.p = pkg;
        UciPtr {
            uci: Arc::clone(&self.uci),
            ptr,
            data: Vec::new(),
        }
    }

    pub fn with_package_name(
        &self,
        name: impl AsRef<str>,
    ) -> Result<UciPtr<PTR_STAGE_PACKAGE, false>> {
        let name = CString::new(name.as_ref())?;
        let mut ptr = self.ptr.clone();
        ptr.target = uci_type_UCI_TYPE_PACKAGE;
        ptr.package = name.as_ptr();
        Ok(UciPtr {
            uci: Arc::clone(&self.uci),
            ptr,
            data: vec![Arc::new(name)],
        })
    }
}

impl UciPtr<PTR_STAGE_PACKAGE, true> {
    pub fn with_section(&self, section: *mut uci_section) -> UciPtr<PTR_STAGE_SECTION, true> {
        let mut ptr = self.ptr.clone();
        ptr.target = uci_type_UCI_TYPE_SECTION;
        ptr.s = section;
        UciPtr {
            uci: Arc::clone(&self.uci),
            ptr,
            data: self.data.iter().map(Arc::clone).collect(),
        }
    }
}

impl<const L: bool> UciPtr<PTR_STAGE_PACKAGE, L> {
    pub fn with_section_name(
        &self,
        name: impl AsRef<str>,
    ) -> Result<UciPtr<PTR_STAGE_SECTION, false>> {
        let name = CString::new(name.as_ref())?;
        let mut ptr = self.ptr.clone();
        ptr.target = uci_type_UCI_TYPE_SECTION;
        ptr.section = name.as_ptr();
        Ok(UciPtr {
            uci: Arc::clone(&self.uci),
            ptr,
            data: self
                .data
                .iter()
                .map(Arc::clone)
                .chain(once(Arc::new(name)))
                .collect(),
        })
    }
}

impl UciPtr<PTR_STAGE_SECTION, true> {
    pub fn with_option(&self, opt: *mut uci_option) -> UciPtr<PTR_STAGE_OPTION, true> {
        let mut ptr = self.ptr.clone();
        ptr.target = uci_type_UCI_TYPE_OPTION;
        ptr.o = opt;
        UciPtr {
            uci: Arc::clone(&self.uci),
            ptr,
            data: self.data.iter().map(Arc::clone).collect(),
        }
    }
}

impl<const L: bool> UciPtr<PTR_STAGE_SECTION, L> {
    pub fn with_option_name(
        &self,
        name: impl AsRef<str>,
    ) -> Result<UciPtr<PTR_STAGE_OPTION, false>> {
        let name = CString::new(name.as_ref())?;
        let mut ptr = self.ptr.clone();
        ptr.target = uci_type_UCI_TYPE_OPTION;
        ptr.option = name.as_ptr();
        Ok(UciPtr {
            uci: Arc::clone(&self.uci),
            ptr,
            data: self
                .data
                .iter()
                .map(Arc::clone)
                .chain(once(Arc::new(name)))
                .collect(),
        })
    }
}

impl<const L: bool> UciPtr<PTR_STAGE_OPTION, L> {
    pub fn with_update(
        &self,
        ptr: uci_ptr,
        extra_data: impl Iterator<Item = CString>,
    ) -> UciPtr<PTR_STAGE_OPTION, true> {
        if ptr.target != uci_type_UCI_TYPE_OPTION {
            panic!("Invalid target, expected Option, got {}", ptr.target);
        }
        if ptr.p.is_null() || ptr.s.is_null() || ptr.o.is_null() {
            panic!("Invalid uci_ptr: not fully looked up");
        }
        if ptr.flags & uci_ptr_UCI_LOOKUP_COMPLETE == 0 {
            panic!("Invalid uci_ptr: lookup not complete");
        }
        UciPtr {
            uci: Arc::clone(&self.uci),
            ptr,
            data: self
                .data
                .iter()
                .map(Arc::clone)
                .chain(extra_data.map(Arc::new))
                .collect(),
        }
    }
}

struct CapturingIter<T, I> {
    inner: I,
    capture: T,
}

impl<T, I> Iterator for CapturingIter<T, I>
where
    I: Iterator,
{
    type Item = I::Item;

    fn next(&mut self) -> StdOption<Self::Item> {
        self.inner.next()
    }
}

impl<const S: usize> UciPtr<S, true> {
    pub fn name(&self) -> Result<&str> {
        Ok(unsafe { CStr::from_ptr((*self.ptr.last).name) }.to_str()?)
    }
}

impl<const S: usize> UciPtr<S, false> {
    pub fn lookup(&self) -> Result<StdOption<UciPtr<S, true>>> {
        let mut ptr = self.ptr.clone();
        let mut uci = self.uci.lock().unwrap();
        let result = libuci_locked!(uci, {
            unsafe { uci_lookup_ptr(uci.ctx, &mut ptr, ptr::null_mut(), true) }
        });
        Ok(handle_error(&mut uci, result)?.map(|_| UciPtr {
            uci: Arc::clone(&self.uci),
            ptr,
            data: self.data.iter().map(Arc::clone).collect(),
        }))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn iterator() {
        // let iter = unsafe { UciListIter::new(list) };
    }
}
