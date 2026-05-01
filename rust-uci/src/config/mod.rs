use std::{
    ffi::{c_char, CStr},
    option::Option as StdOption,
    sync::{Arc, Mutex},
};

use libuci_sys::uci_list_configs;

use crate::{
    error::{Error, Result},
    libuci_locked, Uci, UCI_ERR_NOTFOUND, UCI_OK,
};

mod option;

mod package;
use package::Package;

mod ptr;
use ptr::UciPtr;

mod section;

/// represents the root of the config tree
/// It's the parent structure to [Package]s
pub struct Config {
    uci: Arc<Mutex<Uci>>,
}

impl From<Uci> for Config {
    fn from(uci: Uci) -> Self {
        Self {
            uci: Arc::new(Mutex::new(uci)),
        }
    }
}

struct PackageIter {
    uci: Arc<Mutex<Uci>>,
    ptr: *mut *mut c_char,
}

impl Iterator for PackageIter {
    type Item = Package<false>;

    fn next(&mut self) -> StdOption<Self::Item> {
        if self.ptr.is_null() {
            return None;
        }
        let name_ptr = unsafe { *self.ptr };
        if name_ptr.is_null() {
            return None;
        }
        self.ptr = unsafe { self.ptr.add(1) };
        let name = unsafe { CStr::from_ptr(name_ptr.cast()) }.to_str().unwrap();
        let ptr = UciPtr::new();
        Some(Package::new(
            Arc::clone(&self.uci),
            ptr.with_package_name(name).unwrap(),
        ))
    }
}

impl Config {
    pub fn new() -> Result<Self> {
        Ok(Uci::new()?.into())
    }

    /// return a single [Package] by its name
    /// also works if the package is not defined yet
    pub fn package<'a>(&self, name: impl AsRef<str>) -> Result<Package<false>> {
        let ptr = ptr::UciPtr::new();
        let ptr = ptr.with_package_name(name)?;
        Ok(Package::new(Arc::clone(&self.uci), ptr))
    }

    /// list all [Package]s in the config
    pub fn packages<'a>(&self) -> Result<impl Iterator<Item = Package<false>>> {
        let mut uci = self.uci.lock().unwrap();
        let mut list = std::ptr::null_mut();
        let result = libuci_locked!(uci, { unsafe { uci_list_configs(uci.ctx, &mut list) } });
        let ptr = match handle_error(&mut uci, result)? {
            Some(_) => list,
            None => std::ptr::null_mut(),
        };
        Ok(PackageIter {
            uci: Arc::clone(&self.uci),
            ptr,
        })
    }
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

        let cfg: Config = uci.into();
        let pkg = cfg.package("wireless").unwrap();
        let sect = pkg.section("wifi-device", "pdev0").unwrap();
        let opt = sect.option("channel").unwrap();
        let val = opt.get().unwrap();
        assert_eq!(Some(option::Value::String("auto".into())), val);
    }

    #[test]
    fn list_packages() {
        let (uci, tmp) = setup_uci().unwrap();
        std::fs::write(
            &tmp.path().join("config/wireless"),
            "
            config wifi-device 'pdev0'
                    option channel 'auto'
            ",
        )
        .unwrap();
        std::fs::write(
            &tmp.path().join("config/network"),
            "
            config device 'eth0'
                    option mtu '1280'
            ",
        )
        .unwrap();

        let cfg: Config = uci.into();
        let pkgs: Vec<_> = cfg.packages().unwrap().collect();
        assert_eq!(2, pkgs.len());
        for pkg in pkgs {
            match pkg.name().unwrap() {
                "wireless" => (),
                "network" => (),
                n => panic!("Unexpected name: {}", n),
            }
        }
    }
}
