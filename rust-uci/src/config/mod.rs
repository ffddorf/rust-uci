use std::{ffi::CString, marker::PhantomData, option::Option as StdOption, ptr};

use libuci_sys::{list_to_element, uci_element, uci_list, uci_lookup_next, uci_to_package};

use crate::{
    error::{Error, Result},
    libuci_locked, Uci, UCI_ERR_NOTFOUND, UCI_OK,
};

mod option;

mod package;
use package::{Package, PackageRef};

mod section;

#[derive(Clone)]
pub(crate) enum NameOrRef<N, T> {
    Name(N),
    Ref(T),
}

unsafe trait FromUciElement {
    unsafe fn from_uci_element(elem: *const uci_element) -> Self;
}

struct UciListIter<Item> {
    list: *const uci_list,
    _out: PhantomData<Item>,
}

impl<Item> UciListIter<Item> {
    fn new(list: *const uci_list) -> Self {
        Self {
            list,
            _out: PhantomData,
        }
    }
}

impl<Item> Iterator for UciListIter<Item>
where
    Item: FromUciElement,
{
    type Item = Item;

    fn next(&mut self) -> StdOption<Self::Item> {
        if self.list.is_null() {
            return None;
        }

        let node = unsafe { (*self.list).next };
        if node.is_null() {
            return None;
        }
        if node.cast_const() == self.list {
            return None;
        }

        let elem = unsafe { list_to_element(node.cast_const()) };
        Some(unsafe { Item::from_uci_element(elem) })
    }
}

/// represents the root of the config tree
/// It's the parent structure to [Package]s
pub struct Config {
    uci: Uci,
}

impl Config {
    pub fn new() -> Result<Self> {
        Ok(Self { uci: Uci::new()? })
    }

    /// return a single [Package] by its name
    /// also works if the package is not defined yet
    pub fn package<'a>(&'a mut self, name: impl AsRef<str>) -> Result<Package<'a>> {
        let root = &mut (unsafe { *self.uci.ctx }).root;
        let pkg = lookup_child(&mut self.uci, root, &name)?
            .map(|e| NameOrRef::Ref(unsafe { uci_to_package(e) }.into()))
            .unwrap_or_else(|| NameOrRef::Name(name.as_ref().to_owned()));
        Ok(Package::new(&mut self.uci, pkg))
    }

    /// list all [Package]s in the config
    pub fn packages(&mut self) -> impl Iterator<Item = PackageRef> {
        // Safety:
        // - self.uci.ctx is not null
        let packages = unsafe { (&(*self.uci.ctx).root) as *const _ };
        UciListIter::new(packages)
    }
}

fn lookup_child(
    uci: &mut Uci,
    parent: *mut uci_list,
    name: impl AsRef<str>,
) -> Result<StdOption<*mut uci_element>> {
    let raw = CString::new(name.as_ref())?;
    let mut elem = ptr::null_mut();
    let result = libuci_locked!(uci, {
        unsafe { uci_lookup_next(uci.ctx, &mut elem as *mut _, parent, raw.as_ptr()) }
    });
    match result {
        UCI_OK => (),
        UCI_ERR_NOTFOUND => {
            return Ok(None);
        }
        _ => {
            return Err(Error::Message(
                uci.get_last_error()
                    .unwrap_or_else(|_| String::from("Unknown")),
            ));
        }
    }

    Ok(Some(elem))
}
