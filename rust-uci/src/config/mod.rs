use std::{ffi::CString, option::Option as StdOption, ptr};

use libuci_sys::{
    list_to_element, uci_element, uci_list, uci_load, uci_lookup_next, uci_to_package,
};

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

struct UciListIter {
    list: *const uci_list,
}

impl UciListIter {
    fn new(list: *const uci_list) -> Self {
        Self { list }
    }
}

impl Iterator for UciListIter {
    type Item = *const uci_element;

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

        Some(unsafe { list_to_element(node.cast_const()) })
    }
}

/// represents the root of the config tree
/// It's the parent structure to [Package]s
pub struct Config {
    uci: Uci,
}

impl From<Uci> for Config {
    fn from(uci: Uci) -> Self {
        Self { uci }
    }
}

impl Config {
    pub fn new() -> Result<Self> {
        Ok(Self { uci: Uci::new()? })
    }

    /// return a single [Package] by its name
    /// also works if the package is not defined yet
    pub fn package<'a>(&'a mut self, name: impl AsRef<str>) -> Result<Package<'a>> {
        let root = (unsafe { *self.uci.ctx }).root;
        let lookup = lookup_child(&mut self.uci, root.next, &name)?;
        let lookup = match lookup {
            None => {
                let mut pkg = ptr::null_mut();
                let raw_name = CString::new(name.as_ref())?;
                let result = unsafe { uci_load(self.uci.ctx, raw_name.as_ptr(), &mut pkg) };
                handle_error(&mut self.uci, result)?.map(|_| pkg.cast_const())
            }
            Some(v) => Some(unsafe { uci_to_package(v) }),
        };
        let pkg = lookup
            .map(|e| NameOrRef::Ref(unsafe { PackageRef::new(e) }))
            .unwrap_or_else(|| NameOrRef::Name(name.as_ref().to_owned()));
        Ok(Package::new(&mut self.uci, pkg))
    }

    /// list all [Package]s in the config
    pub fn packages<'a>(&'a mut self) -> impl Iterator<Item = PackageRef<'a>> {
        // todo: this is broken, since packages are loaded on demand
        // needs to use `uci_list_configs`

        // Safety: self.uci.ctx is not null
        let packages = unsafe { (&(*self.uci.ctx).root) as *const _ };
        // Safety: elem is not null
        UciListIter::new(packages).map(|elem| unsafe { PackageRef::new(uci_to_package(elem)) })
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
    Ok(handle_error(uci, result)?.map(|_| elem))
}

fn handle_error(uci: &mut Uci, result: i32) -> Result<Option<()>> {
    match result {
        UCI_OK => Ok(Some(())),
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
}

#[cfg(test)]
mod tests {
    use tempfile::{tempdir, TempDir};

    use super::*;

    fn setup_uci() -> Result<(Uci, TempDir)> {
        let mut uci = Uci::new()?;
        let tmp = tempdir().unwrap();
        let config_dir = tmp.path().join("config");
        let save_dir = tmp.path().join("save");

        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::create_dir_all(&save_dir).unwrap();

        uci.set_config_dir(config_dir.as_os_str().to_str().unwrap())?;
        uci.set_save_dir(save_dir.as_os_str().to_str().unwrap())?;
        Ok((uci, tmp))
    }

    #[test]
    fn get_option() {
        let (uci, tmp) = setup_uci().unwrap();
        let wireless_config_path = tmp.path().join("config/wireless");
        std::fs::write(
            &wireless_config_path,
            "
            config wifi-device 'pdev0'
                    option channel 'auto'

            config wifi-iface 'wifi0'
                    option device 'pdev0'
            ",
        )
        .unwrap();

        let mut cfg: Config = uci.into();
        let mut pkg = cfg.package("wireless").unwrap();
        let mut sect = pkg.section("wifi-device", "pdev0").unwrap();
        let mut opt = sect.option("channel").unwrap();
        let val = opt.get().unwrap();
        assert_eq!(Some(option::Value::String("auto".into())), val);
    }

    // #[test]
    fn list_packages() {
        let mut cfg = Config::new().unwrap();
        let pkgs = cfg.packages();
        for pkg in pkgs {
            println!("{}", pkg.name().unwrap());
        }
    }
}
