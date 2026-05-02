use std::{
    ffi::CString,
    iter::once,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    option::Option as StdOption,
    ptr,
    sync::{Arc, Mutex},
};

use libuci_sys::{
    list_to_element, uci_element, uci_list, uci_lookup_ptr, uci_ptr, uci_type_UCI_TYPE_OPTION,
    uci_type_UCI_TYPE_PACKAGE, uci_type_UCI_TYPE_SECTION, uci_type_UCI_TYPE_UNSPEC,
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

pub(crate) struct UciPtr<const S: usize> {
    ptr: uci_ptr,
    pub uci: Arc<Mutex<Uci>>,
    pub data: Vec<Arc<CString>>,
}

impl<const S: usize> Deref for UciPtr<S> {
    type Target = uci_ptr;

    fn deref(&self) -> &Self::Target {
        &self.ptr
    }
}

impl<const S: usize> DerefMut for UciPtr<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ptr
    }
}

impl UciPtr<PTR_STAGE_INIT> {
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

    pub fn with_package_name(&self, name: impl AsRef<str>) -> Result<UciPtr<PTR_STAGE_PACKAGE>> {
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

impl UciPtr<PTR_STAGE_PACKAGE> {
    pub fn with_section_name(&self, name: impl AsRef<str>) -> Result<UciPtr<PTR_STAGE_SECTION>> {
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

impl UciPtr<PTR_STAGE_SECTION> {
    pub fn with_option_name(&self, name: impl AsRef<str>) -> Result<UciPtr<PTR_STAGE_OPTION>> {
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

impl<const S: usize> UciPtr<S> {
    pub fn lookup(&self) -> Result<StdOption<UciPtr<S>>> {
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
